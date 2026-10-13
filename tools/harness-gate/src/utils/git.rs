use crate::process::CapturedOutput;
use anyhow::{bail, Context, Result};
use std::path::{Component, Path};
use std::time::Duration;

const GIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Run Git in a project root with the standard Harness-Gate timeout.
pub(crate) fn capture<I, S>(project_root: &Path, args: I) -> Result<CapturedOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = command_args(args);
    crate::process::capture("git", &args, project_root, GIT_TIMEOUT)
        .with_context(|| format!("run git {}", args.join(" ")))
}

/// Return NUL-delimited Git path output as UTF-8 repository-relative paths.
pub(crate) fn null_terminated_paths<I, S>(project_root: &Path, args: I) -> Result<Vec<String>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = command_args(args);
    let output = capture(project_root, &args)?;
    if !output.status.success() {
        bail!("git {} failed", args.join(" "));
    }
    parse_null_terminated_paths(&output.stdout)
}

/// Decode Git's `-z` path format without treating newlines as separators.
///
/// Git emits repository-relative paths separated by NUL bytes so filenames
/// containing newlines remain unambiguous. Invalid UTF-8 is rejected because
/// the rest of the workflow represents paths as Rust strings.
fn parse_null_terminated_paths(stdout: &[u8]) -> Result<Vec<String>> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            String::from_utf8(entry.to_vec()).context("Git returned a non-UTF-8 file path")
        })
        .collect()
}

/// Encode a repository-relative filesystem path for Git's index syntax.
pub(crate) fn index_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(
                part.to_str()
                    .context("Git index path must be UTF-8")?
                    .to_string(),
            ),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!(
                    "Git index path must be repository-relative: {}",
                    path.display()
                );
            }
        }
    }
    if parts.is_empty() {
        bail!("Git index path must not be empty");
    }
    Ok(parts.join("/"))
}

/// Read a path from the staged Git snapshot when it exists.
pub(crate) fn staged_file(project_root: &Path, file: &str) -> Result<Option<Vec<u8>>> {
    let args = vec!["show".to_string(), format!(":{file}")];
    let output = capture(project_root, args).with_context(|| format!("read staged file {file}"))?;
    Ok(output.status.success().then_some(output.stdout))
}

/// Return the size of a path in the staged Git snapshot when it exists.
pub(crate) fn staged_file_size(project_root: &Path, file: &str) -> Result<Option<u64>> {
    let args = vec!["cat-file".to_string(), "-s".into(), format!(":{file}")];
    let output =
        capture(project_root, args).with_context(|| format!("inspect staged file {file}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    let size = std::str::from_utf8(&output.stdout)
        .context("Git returned a non-UTF-8 staged file size")?
        .trim()
        .parse::<u64>()
        .with_context(|| format!("parse staged file size for {file}"))?;
    Ok(Some(size))
}

fn command_args<I, S>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{index_path, parse_null_terminated_paths};
    use std::path::Path;

    #[test]
    fn index_paths_use_git_separators_on_every_platform() {
        let path = Path::new(".harness-gate").join("secrets.toml");

        assert_eq!(
            index_path(&path).expect("valid repository path"),
            ".harness-gate/secrets.toml"
        );
    }

    #[test]
    fn index_paths_reject_parent_traversal() {
        let error = index_path(Path::new("../secrets.toml"))
            .expect_err("parent traversal must not reach Git");

        assert!(error.to_string().contains("repository-relative"));
    }

    #[test]
    fn parses_nul_delimited_paths_without_splitting_newlines() {
        let paths = parse_null_terminated_paths(b"src/line\nname.rs\0Cargo.toml\0")
            .expect("valid Git paths");
        assert_eq!(paths, ["src/line\nname.rs", "Cargo.toml"]);
    }

    #[test]
    fn rejects_non_utf8_paths() {
        let error =
            parse_null_terminated_paths(b"src/\xff.rs\0").expect_err("non-UTF-8 path must fail");
        assert!(error.to_string().contains("non-UTF-8"));
    }
}
