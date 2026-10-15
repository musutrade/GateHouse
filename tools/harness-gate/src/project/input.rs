use crate::utils::git;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static SNAPSHOT_COUNTER: AtomicU64 = AtomicU64::new(1);

/// The source interpretation attached to one verification invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum InputMode {
    WorkingTree,
    Staged,
    Base,
    All,
}

impl InputMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::WorkingTree => "working-tree",
            Self::Staged => "staged",
            Self::Base => "base",
            Self::All => "all",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct InvocationInput {
    pub(crate) mode: InputMode,
    pub(crate) project_identity: String,
    pub(crate) source_identity: String,
    /// Optional HEAD context sampled at discovery, distinct from snapshot identity.
    pub(crate) commit: String,
    pub(crate) execution_root: PathBuf,
    pub(crate) configuration_digest: String,
    #[serde(skip)]
    snapshot: Option<Arc<SnapshotGuard>>,
}

#[derive(Debug)]
struct SnapshotGuard {
    root: PathBuf,
}

impl Drop for SnapshotGuard {
    fn drop(&mut self) {
        // This path is created exclusively by `materialize_staged`; never
        // follow a link while removing a failed or completed snapshot.
        if fs::symlink_metadata(&self.root)
            .map(|metadata| metadata.file_type().is_dir() && !metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

impl InvocationInput {
    /// A private configuration view, not an executable source snapshot. Empty
    /// placeholders preserve indexed path existence; only configuration/policy
    /// blobs are read. Source bytes and signed request bodies are never needed.
    pub(crate) fn inspect_staged(root: &Path, config: &Path, commit: &str) -> Result<Self> {
        let index = git::capture_readonly(root, ["ls-files", "--stage", "-z"])?;
        if !index.status.success() {
            bail!("cannot inspect staged Git index");
        }
        let snapshot_root = allocate_snapshot_root()?;
        let guard = Arc::new(SnapshotGuard {
            root: snapshot_root.clone(),
        });
        let entries = inspect_index_entries(&snapshot_root, &index.stdout)?;
        hydrate_index_config(root, &snapshot_root, &entries, &git::index_path(config)?)?;
        if entries.contains_key(crate::plugins::LOCK_PATH) {
            hydrate_index_config(root, &snapshot_root, &entries, crate::plugins::LOCK_PATH)?;
        }
        let quality_path = crate::config::quality::QUALITY_CONFIG_PATH;
        if entries.contains_key(quality_path) {
            hydrate_index_config(root, &snapshot_root, &entries, quality_path)?;
            let quality: toml::Value =
                toml::from_str(&fs::read_to_string(snapshot_root.join(quality_path))?)?;
            if let Some(policies) = quality.get("policies").and_then(toml::Value::as_table) {
                for policy in policies.values() {
                    if let Some(file) = policy.get("policy_file").and_then(toml::Value::as_str) {
                        hydrate_index_config(root, &snapshot_root, &entries, file)?;
                    }
                }
            }
        }
        Ok(Self {
            mode: InputMode::Staged,
            project_identity: root.to_string_lossy().into_owned(),
            source_identity: format!("git-index-metadata:{}", digest_bytes(&index.stdout)),
            commit: commit.into(),
            execution_root: snapshot_root,
            configuration_digest: String::new(),
            snapshot: Some(guard),
        })
    }

    pub(crate) fn working_tree(
        repository_root: &Path,
        config_bytes: &[u8],
        readonly: bool,
    ) -> Result<Self> {
        let root = repository_root
            .canonicalize()
            .with_context(|| format!("resolve project root {}", repository_root.display()))?;
        // Preview needs effective configuration, not an executable source
        // identity. Avoid a HEAD object probe that could hydrate a partial clone.
        let head = if readonly {
            "not-sampled".into()
        } else {
            optional_head(&root)?
        };
        Ok(Self {
            mode: InputMode::WorkingTree,
            project_identity: root.to_string_lossy().into_owned(),
            source_identity: format!("working-tree:{head}"),
            commit: head,
            execution_root: root,
            configuration_digest: digest_bytes(config_bytes),
            snapshot: None,
        })
    }

    pub(crate) fn materialize_staged(
        repository_root: &Path,
        configuration_path: &Path,
        commit: &str,
    ) -> Result<Self> {
        let root = repository_root
            .canonicalize()
            .with_context(|| format!("resolve project root {}", repository_root.display()))?;
        let tree = git::capture(&root, ["write-tree"]).context("materialize staged Git index")?;
        if !tree.status.success() {
            bail!(
                "cannot materialize staged Git index: {}",
                String::from_utf8_lossy(&tree.stderr).trim()
            );
        }
        let tree_id = String::from_utf8(tree.stdout)
            .context("Git returned a non-UTF-8 staged tree identity")?
            .trim()
            .to_string();
        if tree_id.is_empty() {
            bail!("Git returned an empty staged tree identity");
        }

        let snapshot_root = allocate_snapshot_root()?;
        let prefix = format!("{}/", snapshot_root.to_string_lossy());
        let prefix_argument = format!("--prefix={prefix}");
        let checkout = git::capture(&root, ["checkout-index", "--all", prefix_argument.as_str()])?;
        if !checkout.status.success() {
            let _ = fs::remove_dir_all(&snapshot_root);
            bail!(
                "cannot materialize staged Git index: {}",
                String::from_utf8_lossy(&checkout.stderr).trim()
            );
        }
        let guard = Arc::new(SnapshotGuard {
            root: snapshot_root.clone(),
        });
        reject_snapshot_links(&snapshot_root)?;
        let config_bytes = fs::read(snapshot_root.join(configuration_path)).with_context(|| {
            format!(
                "read staged workflow configuration {}",
                configuration_path.display()
            )
        })?;
        let config_digest = digest_bytes(&config_bytes);
        Ok(Self {
            mode: InputMode::Staged,
            project_identity: root.to_string_lossy().into_owned(),
            source_identity: format!("git-tree:{tree_id}"),
            commit: commit.to_string(),
            execution_root: snapshot_root,
            configuration_digest: config_digest,
            snapshot: Some(guard),
        })
    }

    pub(crate) fn is_snapshot(&self) -> bool {
        self.snapshot.is_some()
    }
}

fn inspect_index_entries(
    root: &Path,
    bytes: &[u8],
) -> Result<std::collections::BTreeMap<String, String>> {
    let mut entries = std::collections::BTreeMap::new();
    for entry in bytes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let entry = std::str::from_utf8(entry).context("non-UTF-8 index entry")?;
        let (metadata, path) = entry.split_once('\t').context("malformed index entry")?;
        let fields = metadata.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 3 || fields[2] != "0" || !matches!(fields[0], "100644" | "100755") {
            bail!("staged plan requires regular, resolved index entries: {path:?}");
        }
        let destination = super::resolve_repo_path(root, Path::new(path), "indexed path", false)?;
        fs::create_dir_all(destination.parent().context("indexed path has no parent")?)?;
        fs::File::create(destination)?;
        entries.insert(path.into(), fields[1].into());
    }
    Ok(entries)
}

fn hydrate_index_config(
    repository: &Path,
    snapshot: &Path,
    entries: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Result<()> {
    let index_name = git::index_path(Path::new(name))?;
    let id = entries
        .get(&index_name)
        .with_context(|| format!("required staged configuration {name:?} is missing"))?;
    let destination =
        super::resolve_repo_path(snapshot, Path::new(name), "staged configuration", true)?;
    reject_plan_config_filters(repository, name)?;
    let blob = git::capture_readonly(repository, ["cat-file", "blob", id])?;
    if !blob.status.success() {
        bail!("cannot read staged configuration {name:?}");
    }
    fs::write(destination, blob.stdout)?;
    Ok(())
}

/// Execution's checkout-index may transform configuration with a user filter.
/// A read-only preview cannot execute that filter or promise its output, so
/// refuse this case explicitly rather than explain a different configuration.
fn reject_plan_config_filters(root: &Path, name: &str) -> Result<()> {
    for cached in [false, true] {
        let mut args = vec!["check-attr", "-z"];
        if cached {
            args.push("--cached");
        }
        args.extend(["filter", "working-tree-encoding", "ident", "--", name]);
        let attributes = git::capture_readonly(root, args)?;
        if !attributes.status.success() {
            bail!("cannot inspect staged configuration filters for {name:?}");
        }
        let fields = attributes
            .stdout
            .split(|byte| *byte == 0)
            .collect::<Vec<_>>();
        if fields.len() != 10 {
            bail!("malformed Git attribute response");
        }
        for entry in fields[..9].as_chunks::<3>().0 {
            if !matches!(entry[2], b"unspecified" | b"unset") {
                bail!("staged configuration {name:?} has a checkout filter/encoding/ident transformation; a read-only plan cannot determine its output");
            }
        }
    }
    Ok(())
}

/// Optional checkout context, never a substitute for required source evidence.
/// Sample once at discovery with a small budget; cancellation remains an error.
fn optional_head(project_root: &Path) -> Result<String> {
    if crate::process::cancelled() {
        bail!("Git metadata probe was cancelled");
    }
    let output = crate::process::capture_with_limits(
        "git",
        &[
            "rev-parse".into(),
            "--verify".into(),
            "HEAD^{commit}".into(),
        ],
        project_root,
        Duration::from_secs(1),
        crate::process::CaptureLimits {
            stdout_bytes: 4096,
            stderr_bytes: 4096,
            total_bytes: 8192,
            reader_deadline: Duration::from_millis(200),
        },
    );
    if crate::process::cancelled() {
        bail!("Git metadata probe was cancelled");
    }
    Ok(output
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| parse_commit(&output.stdout))
        .unwrap_or_else(|| "unknown".into()))
}

fn parse_commit(stdout: &[u8]) -> Option<String> {
    let value = std::str::from_utf8(stdout).ok()?.trim();
    (matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_string())
}

fn allocate_snapshot_root() -> Result<PathBuf> {
    // Configuration containment checks compare canonical paths. Resolve the
    // temporary parent too (for example, /var -> /private/var on macOS).
    let base = std::env::temp_dir()
        .canonicalize()
        .context("resolve staged snapshot temporary directory")?;
    for _ in 0..64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let id = SNAPSHOT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!(
            "harness-gate-staged-{}-{}-{}",
            std::process::id(),
            now.as_nanos(),
            id
        ));
        #[cfg(unix)]
        let created = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700).create(&path)
        };
        #[cfg(not(unix))]
        let created = fs::create_dir(&path);
        match created {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("create staged snapshot {}", path.display()));
            }
        }
    }
    bail!("could not allocate a collision-free staged snapshot directory")
}

fn reject_snapshot_links(root: &Path) -> Result<()> {
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)
            .with_context(|| format!("read staged snapshot {}", directory.display()))?
        {
            let entry = entry.context("read staged snapshot entry")?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .with_context(|| format!("inspect staged snapshot entry {}", path.display()))?;
            if metadata.file_type().is_symlink() {
                let _ = fs::remove_dir_all(root);
                bail!(
                    "staged snapshot contains a symbolic link: {}",
                    path.display()
                );
            }
            if metadata.is_dir() {
                directories.push(path);
            } else if !metadata.is_file() {
                let _ = fs::remove_dir_all(root);
                bail!(
                    "staged snapshot contains a non-regular entry: {}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("sha256:{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::parse_commit;

    #[test]
    fn readonly_index_view_rejects_conflicts_links_and_traversal_without_git_writes() {
        let root = tempfile::tempdir().unwrap();
        let canonical_root = root.path().canonicalize().unwrap();
        for bytes in [
            &b"100644 deadbeef 1\tconflict\0"[..],
            &b"120000 deadbeef 0\tlink\0"[..],
            &b"160000 deadbeef 0\tsubmodule\0"[..],
            &b"100644 deadbeef 0\t../escape\0"[..],
            &b"100644 deadbeef 0\t/absolute\0"[..],
            &b"missing-tab\0"[..],
            &b"\xff\0"[..],
        ] {
            assert!(super::inspect_index_entries(&canonical_root, bytes).is_err());
        }
        let entries = super::inspect_index_entries(
            &canonical_root,
            b"100755 deadbeef 0\tdir/name with spaces.txt\0",
        )
        .unwrap();
        assert_eq!(entries["dir/name with spaces.txt"], "deadbeef");
        assert!(canonical_root.join("dir/name with spaces.txt").is_file());
        #[cfg(unix)]
        {
            let entries = super::inspect_index_entries(
                &canonical_root,
                b"100755 deadbeef 0\tdir/name\nwith tab\t.txt\0",
            )
            .unwrap();
            assert_eq!(entries["dir/name\nwith tab\t.txt"], "deadbeef");
        }
        assert!(
            super::hydrate_index_config(root.path(), root.path(), &entries, "missing").is_err()
        );
    }

    #[test]
    fn optional_commit_accepts_only_git_object_ids() {
        for size in [40, 64] {
            let sha = "a".repeat(size);
            assert_eq!(
                parse_commit(format!("{sha}\n").as_bytes()),
                Some(sha.clone())
            );
            assert_eq!(parse_commit(format!("{sha}\r\n").as_bytes()), Some(sha));
        }
        for output in [
            vec![],
            b"HEAD\n".to_vec(),
            b"not-a-commit\n".to_vec(),
            "a".repeat(39).into_bytes(),
            "a".repeat(41).into_bytes(),
            "g".repeat(40).into_bytes(),
            format!("{}\n{}", "a".repeat(40), "b".repeat(40)).into_bytes(),
            vec![0xff; 40],
        ] {
            assert_eq!(parse_commit(&output), None, "{output:?}");
        }
    }
}
