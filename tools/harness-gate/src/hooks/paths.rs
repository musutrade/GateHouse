use anyhow::{ensure, Context, Result};
use std::path::{Path, PathBuf};

fn git_path(root: &Path, flag: &str, tail: Option<&str>) -> Result<PathBuf> {
    let mut args = vec!["rev-parse", flag];
    if let Some(tail) = tail {
        args.push(tail);
    }
    let output = crate::utils::git::capture_readonly(root, args)?;
    ensure!(output.status.success(), "cannot resolve Git hook directory");
    let text = std::str::from_utf8(&output.stdout)?.trim_end_matches(['\r', '\n']);
    ensure!(
        !text.is_empty() && !text.contains(['\r', '\n']),
        "unsupported Git path output"
    );
    let path = PathBuf::from(text);
    Ok(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

pub(super) fn resolve(root: &Path) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let root = root.as_path();
    let common_path = git_path(root, "--git-common-dir", None)?;
    ensure!(
        !std::fs::symlink_metadata(&common_path)?
            .file_type()
            .is_symlink(),
        "Git common directory cannot be a symlink"
    );
    let common = common_path.canonicalize()?;
    let expected = common.join("hooks");
    let actual = git_path(root, "--git-path", Some("hooks"))?;
    let metadata =
        std::fs::symlink_metadata(&actual).context("hooks directory must already exist")?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "hooks directory must be a real directory, not a symlink"
    );
    // Check the path before canonicalization so aliases cannot authorize external hooksPath.
    ensure!(
        actual == expected || actual == root.join(".git/hooks"),
        "non-default core.hooksPath is not managed; keep your existing hook manager"
    );
    ensure!(
        actual.canonicalize()? == expected,
        "hook path does not resolve to the default common directory"
    );
    Ok(expected)
}
