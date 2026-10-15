//! Explicit interactive application of a freshly generated repair plan.
use super::{plan, safe_text, Plan};
use crate::config::{read_config_source, FlowConfig, DEFAULT_CONFIG_PATH, MAX_CONFIG_BYTES};
use anyhow::{ensure, Context, Result};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn run(root: &Path, override_path: Option<PathBuf>) -> Result<bool> {
    ensure!(
        io::stdin().is_terminal() && io::stderr().is_terminal(),
        "interactive repairs require terminal input and stderr; use config suggest to inspect a plan"
    );
    let root = normalize_prefix(&root.canonicalize().context("resolve project root")?);
    let input = override_path.unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    let path = resolve_path(&root, input)?;
    validate_path(&root, &path)?;
    let _lock = RepairLock::acquire(&path)?;
    let source = read_config_source(&path)?;
    let plan = plan(&source, &path, &root);
    let Some(replacement) = replacement(&source, &plan)? else {
        eprintln!("No candidate edits available; inspect the manual suggestions.");
        return Ok(plan.configuration.valid);
    };
    validate_replacement(&replacement, &path, &root)?;
    for suggestion in plan.suggestions.iter().filter(|s| s.edit.is_some()) {
        eprintln!(
            "{}: {}",
            safe_text(&suggestion.field),
            safe_text(&suggestion.message)
        );
        if let Some(edit) = &suggestion.edit {
            eprintln!(
                "  replace bytes {}..{} with {}",
                edit.start,
                edit.end,
                safe_text(&edit.replacement)
            );
        }
        eprintln!("  applicability: maybe incorrect; confirm the intended log filename");
    }
    eprint!("Apply these candidate edits? [y/N] ");
    io::stderr().flush()?;
    if !confirm(&mut io::stdin().lock())? {
        eprintln!("No changes applied.");
        return Ok(false);
    }
    publish(&root, &path, &source, &replacement)?;
    eprintln!("Applied workflow repairs. Run config check for complete project validation.");
    Ok(true)
}

fn confirm(reader: &mut impl BufRead) -> Result<bool> {
    let mut answer = String::new();
    reader.take(128).read_line(&mut answer)?;
    ensure!(answer.len() < 128, "repair confirmation is too long");
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

fn replacement(source: &str, plan: &Plan) -> Result<Option<String>> {
    let mut edits = plan
        .suggestions
        .iter()
        .filter_map(|s| s.edit.as_ref())
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return Ok(None);
    }
    edits.sort_by_key(|edit| edit.start);
    let mut previous_end = 0;
    for edit in &edits {
        ensure!(
            edit.start >= previous_end && source.get(edit.start..edit.end).is_some(),
            "repair ranges overlap or do not address original UTF-8 text"
        );
        previous_end = edit.end;
    }
    let mut result = source.to_string();
    for edit in edits.into_iter().rev() {
        result.replace_range(edit.start..edit.end, &edit.replacement);
    }
    Ok(Some(result))
}

fn resolve_path(root: &Path, input: PathBuf) -> Result<PathBuf> {
    ensure!(
        !input
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir)),
        "repair file may not contain parent traversal"
    );
    let path = if input.is_absolute() {
        input
    } else {
        root.join(input)
    };
    Ok(normalize_prefix(&path))
}

#[cfg(windows)]
fn normalize_prefix(path: &Path) -> PathBuf {
    use std::path::{Component, Prefix};
    let mut components = path.components();
    let prefix = match components.next() {
        Some(Component::Prefix(prefix)) => prefix.kind(),
        _ => return path.to_path_buf(),
    };
    let mut normalized = match prefix {
        Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => {
            PathBuf::from(format!(r"\\?\{}:", char::from(drive.to_ascii_uppercase())))
        }
        Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
            let mut prefix = std::ffi::OsString::from(r"\\?\UNC\");
            prefix.push(server);
            prefix.push(r"\");
            prefix.push(share);
            PathBuf::from(prefix)
        }
        _ => return path.to_path_buf(),
    };
    // Append the untouched suffix as OS text: PathBuf::push on a verbatim
    // Windows path would normalize away parent traversal before validation.
    if matches!(prefix, Prefix::Disk(_) | Prefix::UNC(_, _)) {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        let suffix = components
            .as_path()
            .as_os_str()
            .encode_wide()
            .map(|unit| {
                if unit == u16::from(b'/') {
                    u16::from(b'\\')
                } else {
                    unit
                }
            })
            .collect::<Vec<_>>();
        normalized
            .as_mut_os_string()
            .push(std::ffi::OsString::from_wide(&suffix));
    } else {
        normalized.as_mut_os_string().push(components.as_path());
    }
    normalized
}

#[cfg(not(windows))]
fn normalize_prefix(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn validate_path(root: &Path, path: &Path) -> Result<()> {
    ensure!(
        path.starts_with(root),
        "repair file must be inside the project root"
    );
    ensure!(
        !path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir)),
        "repair file may not contain parent traversal"
    );
    let parent = path.parent().context("repair file has no parent")?;
    let relative = parent.strip_prefix(root)?;
    let mut component_path = root.to_path_buf();
    for component in relative.components() {
        component_path.push(component);
        let metadata = fs::symlink_metadata(&component_path)?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "repair parent must be a directory, not a symbolic link"
        );
    }
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "repair file must be a regular file, not a symbolic link"
    );
    ensure!(
        normalize_prefix(&path.canonicalize()?).starts_with(root),
        "repair file escapes the project root"
    );
    Ok(())
}

fn validate_replacement(source: &str, path: &Path, root: &Path) -> Result<()> {
    ensure!(
        source.len() as u64 <= MAX_CONFIG_BYTES,
        "candidate exceeds the {} byte configuration limit; no edits applied",
        MAX_CONFIG_BYTES
    );
    FlowConfig::from_source_with_diagnostics(source, Some(path), Some(root))
        .map_err(anyhow::Error::from)
        .context("candidate still fails workflow validation; resolve manual diagnostics first")?;
    Ok(())
}

fn publish(root: &Path, path: &Path, original: &str, replacement: &str) -> Result<()> {
    validate_path(root, path)?;
    ensure!(
        read_config_source(path)? == original,
        "configuration changed since the preview; no edits applied"
    );
    validate_replacement(replacement, path, root)?;
    let metadata = fs::metadata(path)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(path.parent().context("missing repair parent")?)?;
    temporary
        .as_file()
        .set_permissions(metadata.permissions())?;
    temporary.write_all(replacement.as_bytes())?;
    temporary.as_file().sync_all()?;
    validate_path(root, path)?;
    ensure!(
        read_config_source(path)? == original,
        "configuration changed before publication; no edits applied"
    );
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .context("publish configuration repair")?;
    Ok(())
}

struct RepairLock {
    path: PathBuf,
    token: Vec<u8>,
}

impl RepairLock {
    fn acquire(target: &Path) -> Result<Self> {
        let name = target
            .file_name()
            .context("repair file has no filename")?
            .to_string_lossy();
        let path = target.with_file_name(format!(".{name}.repair.lock"));
        let token = format!(
            "{}:{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        )
        .into_bytes();
        let mut file = OpenOptions::new().write(true).create_new(true).open(&path)
            .context("repair lock already exists or cannot be created; inspect the existing repair before retrying")?;
        let guard = Self { path, token };
        file.write_all(&guard.token)?;
        Ok(guard)
    }
}

impl Drop for RepairLock {
    fn drop(&mut self) {
        let regular = fs::symlink_metadata(&self.path).is_ok_and(|m| {
            m.is_file() && !m.file_type().is_symlink() && m.len() == self.token.len() as u64
        });
        let mut value = Vec::new();
        let matches = regular
            && fs::File::open(&self.path)
                .and_then(|file| {
                    file.take(self.token.len() as u64 + 1)
                        .read_to_end(&mut value)
                })
                .is_ok()
            && value == self.token;
        if matches {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_defaults_to_no_and_bounds_input() {
        for input in ["", "\n", "n\n", "true\n", "yes please\n"] {
            assert!(!confirm(&mut input.as_bytes()).unwrap());
        }
        for input in ["y\n", "Y\n", "yes\n", "YES\n"] {
            assert!(confirm(&mut input.as_bytes()).unwrap());
        }
        assert!(confirm(&mut "y".repeat(128).as_bytes()).is_err());
    }

    #[test]
    fn lock_excludes_other_repairs_and_preserves_foreign_contents() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("flow.toml");
        let guard = RepairLock::acquire(&target).unwrap();
        assert!(RepairLock::acquire(&target).is_err());
        let lock = guard.path.clone();
        drop(guard);
        assert!(!lock.exists());
        let guard = RepairLock::acquire(&target).unwrap();
        fs::write(&lock, "foreign").unwrap();
        drop(guard);
        assert_eq!(fs::read_to_string(lock).unwrap(), "foreign");
    }

    #[test]
    fn invalid_paths_do_not_create_directories() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        assert!(validate_path(&root, &root.join("missing/flow.toml")).is_err());
        assert!(!root.join("missing").exists());
        assert!(validate_path(&root, &root.join("../flow.toml")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_in_any_project_parent() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("real/nested")).unwrap();
        fs::write(root.join("real/nested/flow.toml"), "original").unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join("alias")).unwrap();
        assert!(validate_path(&root, &root.join("alias/nested/flow.toml")).is_err());
        assert!(validate_path(&root, &root.join("real/nested/flow.toml")).is_ok());
    }
    const FLOW: &str = r#"
version = 2
[project]
name = "boundary"
default_profile = "full"
hook_profile = "hook"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "all"
[[scope.rules]]
patterns = ["**"]
components = ["project"]
[[steps]]
id = "probe"
label = "probe"
component = "project"
profiles = ["full", "hook"]
program = "sh"
args = ["probe.sh", "original"]
cwd = "{root}"
log = "probe.log"
timeout_secs = 20
"#;

    #[test]
    fn publish_rechecks_input_and_candidate_then_preserves_other_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let path = root.join("flow.toml");
        let original = FLOW.replace("probe.log", "bad/path.log");
        fs::write(&path, &original).unwrap();
        let proposal = plan(&original, &path, &root);
        let repaired = replacement(&original, &proposal).unwrap().unwrap();
        assert_eq!(
            repaired,
            original.replace("bad/path.log", "step-0-repaired.log")
        );
        assert!(publish(&root, &path, "stale original", &repaired).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert!(publish(&root, &path, &original, "not valid TOML = [").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        publish(&root, &path, &original, &repaired).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), repaired);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }

    #[test]
    fn replacement_rejects_invalid_ranges_and_leaves_valid_workflow_alone() {
        let path = Path::new("flow.toml");
        assert!(replacement(FLOW, &plan(FLOW, path, Path::new(".")))
            .unwrap()
            .is_none());
        let original = FLOW.replace("probe.log", "bad/path.log");
        let mut proposal = plan(&original, path, Path::new("."));
        proposal.suggestions[0].edit.as_mut().unwrap().end = original.len() + 1;
        assert!(replacement(&original, &proposal).is_err());
    }
    #[test]
    fn replacement_rejects_overlap_and_non_utf8_boundaries() {
        let original = FLOW.replace("probe.log", "bad/path.log");
        let mut proposal = plan(&original, Path::new("flow.toml"), Path::new("."));
        let edit = proposal.suggestions[0].edit.as_ref().unwrap();
        proposal.suggestions.push(super::super::Suggestion {
            diagnostic_id: "test".into(),
            field: "test".into(),
            message: "test".into(),
            applicability: super::super::Applicability::MaybeIncorrect,
            edit: Some(super::super::TextEdit {
                start: edit.start,
                end: edit.end,
                replacement: "x".into(),
            }),
        });
        assert!(replacement(&original, &proposal).is_err());
        proposal.suggestions.truncate(1);
        let edit = proposal.suggestions[0].edit.as_mut().unwrap();
        edit.start = 1;
        edit.end = 2;
        assert!(replacement("é", &proposal).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn publication_rejects_final_symlink_and_preserves_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let path = root.join("flow.toml");
        let original = FLOW.replace("probe.log", "bad/path.log");
        fs::write(&path, &original).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let link = root.join("link.toml");
        symlink(&path, &link).unwrap();
        assert!(publish(&root, &link, &original, FLOW).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        publish(&root, &path, &original, FLOW).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
    #[test]
    fn growing_candidate_cannot_publish_an_unloadable_configuration() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let path = root.join("flow.toml");
        let mut original = FLOW.replace("probe.log", "bad/path.log");
        original.push_str("\n#");
        original.push_str(&"x".repeat(MAX_CONFIG_BYTES as usize - original.len()));
        fs::write(&path, &original).unwrap();
        assert_eq!(read_config_source(&path).unwrap(), original);
        let candidate = original.replace("bad/path.log", "step-0-repaired.log");
        {
            let _lock = RepairLock::acquire(&path).unwrap();
            assert!(publish(&root, &path, &original, &candidate).is_err());
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        let bounded = &candidate[..MAX_CONFIG_BYTES as usize];
        validate_replacement(bounded, &path, &root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_prefix_normalization_preserves_containment_and_traversal() {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        let directory = tempfile::tempdir().unwrap();
        let root = normalize_prefix(&directory.path().canonicalize().unwrap());
        let path = root.join("flow.toml");
        fs::write(&path, FLOW).unwrap();
        let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
        let ordinary = PathBuf::from(std::ffi::OsString::from_wide(&units[4..]));
        assert_eq!(normalize_prefix(&ordinary), path);
        let forward = units[4..]
            .iter()
            .map(|unit| {
                if *unit == u16::from(b'\\') {
                    u16::from(b'/')
                } else {
                    *unit
                }
            })
            .collect::<Vec<_>>();
        let forward = PathBuf::from(std::ffi::OsString::from_wide(&forward));
        assert!(validate_path(&root, &resolve_path(&root, forward).unwrap()).is_ok());
        assert_eq!(
            normalize_prefix(Path::new(r"C:/repo\nested/file.toml")),
            PathBuf::from(r"\\?\C:\repo\nested\file.toml")
        );
        assert_eq!(
            normalize_prefix(Path::new(r"\\server\share/dir\file")),
            PathBuf::from(r"\\?\UNC\server\share\dir\file")
        );
        assert!(normalize_prefix(Path::new(r"C:\repo\nested\..\flow.toml"))
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir)));
        assert!(validate_path(&root, &resolve_path(&root, ordinary).unwrap()).is_ok());
        assert!(resolve_path(&root, PathBuf::from("nested/../flow.toml")).is_err());
        assert_eq!(
            resolve_path(&root, PathBuf::from("flow.toml")).unwrap(),
            path
        );
        assert_eq!(
            normalize_prefix(Path::new("C:/repo/flow.toml")),
            PathBuf::from(r"\\?\C:\repo\flow.toml")
        );
        assert!(validate_path(&root, &normalize_prefix(&root.join("../flow.toml"))).is_err());
        assert!(validate_path(
            &root,
            &normalize_prefix(&root.parent().unwrap().join("other.toml"))
        )
        .is_err());
        assert_eq!(
            normalize_prefix(Path::new(r"\\server\share\dir\file")),
            PathBuf::from(r"\\?\UNC\server\share\dir\file")
        );
    }
}
