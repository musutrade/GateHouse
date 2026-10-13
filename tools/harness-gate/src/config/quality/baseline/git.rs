//! Read immutable Git objects; never checkout, reset, or register a worktree.
use anyhow::{ensure, Context, Result};
use std::{fs, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .context("run baseline Git reader")?;
    ensure!(
        output.status.success(),
        "baseline git {}: {}",
        args[0],
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

pub(super) fn commit(root: &Path, reference: &str) -> Result<String> {
    let bytes = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{reference}^{{commit}}"),
        ],
    )?;
    let commit = String::from_utf8(bytes)?.trim().to_owned();
    ensure!(
        commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()),
        "unsupported Git object identity"
    );
    Ok(commit)
}

pub(super) fn resolve(
    root: &Path,
    reference: &str,
    merge_base: bool,
    head: &str,
) -> Result<String> {
    let base = commit(root, reference)?;
    if !merge_base {
        return Ok(base);
    }
    let output = String::from_utf8(git(root, &["merge-base", "--all", head, &base])?)?;
    let commits: Vec<_> = output.lines().collect();
    ensure!(
        commits.len() == 1,
        "baseline requires one unambiguous merge base"
    );
    commit(root, commits[0])
}

pub(super) fn materialize(root: &Path, commit: &str, destination: &Path) -> Result<()> {
    let tree = git(root, &["ls-tree", "-r", "-z", "--full-tree", commit])?;
    for entry in tree.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = std::str::from_utf8(entry).context("baseline requires UTF-8 Git paths")?;
        let (metadata, name) = entry.split_once('\t').context("invalid Git tree entry")?;
        let fields: Vec<_> = metadata.split(' ').collect();
        ensure!(
            fields.len() == 3 && fields[1] == "blob" && matches!(fields[0], "100644" | "100755"),
            "baseline snapshot does not support symlinks or submodules: {name}"
        );
        super::relative(name)?;
        let target = destination.join(name);
        fs::create_dir_all(target.parent().context("snapshot parent")?)?;
        fs::write(&target, git(root, &["cat-file", "blob", fields[2]])?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                target,
                fs::Permissions::from_mode(if fields[0] == "100755" { 0o755 } else { 0o644 }),
            )?;
        }
    }
    Ok(())
}
