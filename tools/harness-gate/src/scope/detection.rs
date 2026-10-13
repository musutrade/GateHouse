use super::{ScopeError, ScopeMode, ScopeResult};
use crate::config::UnmatchedScope;
use crate::project::Project;
use crate::utils::git;
use anyhow::{bail, Result};
use std::collections::BTreeSet;

/// Select changed paths for a mode, de-duplicate them, then classify them
/// using the project scope rules. Git command selection belongs here; decoding
/// Git's NUL-delimited output is delegated to [`crate::utils::git`].
pub fn detect(project: &Project, mode: &ScopeMode) -> std::result::Result<ScopeResult, ScopeError> {
    if matches!(mode, ScopeMode::All) {
        return Ok(ScopeResult::all(project));
    }

    ensure_git_worktree(project).map_err(ScopeError::git)?;
    let mut paths = BTreeSet::new();
    let mode_label = match mode {
        ScopeMode::WorkingTree => {
            paths.extend(
                git::null_terminated_paths(&project.root, ["diff", "--name-only", "-z"])
                    .map_err(ScopeError::git)?,
            );
            paths.extend(
                git::null_terminated_paths(
                    &project.root,
                    ["diff", "--cached", "--name-only", "-z"],
                )
                .map_err(ScopeError::git)?,
            );
            paths.extend(
                git::null_terminated_paths(
                    &project.root,
                    ["ls-files", "--others", "--exclude-standard", "-z"],
                )
                .map_err(ScopeError::git)?,
            );
            "working-tree".to_string()
        }
        ScopeMode::Staged => {
            paths.extend(
                git::null_terminated_paths(
                    &project.root,
                    ["diff", "--cached", "--name-only", "-z"],
                )
                .map_err(ScopeError::git)?,
            );
            "staged".to_string()
        }
        ScopeMode::Base(reference) => {
            let revision = format!("{reference}^{{commit}}");
            let output = git::capture(&project.root, ["rev-parse", "--verify", revision.as_str()])
                .map_err(ScopeError::git)?;
            if !output.status.success() {
                return Err(ScopeError::Git {
                    message: format!("base reference does not exist: {reference}"),
                });
            }
            let range = format!("{reference}...HEAD");
            paths.extend(
                git::null_terminated_paths(
                    &project.root,
                    ["diff", "--name-only", "-z", range.as_str()],
                )
                .map_err(ScopeError::git)?,
            );
            format!("base:{reference}")
        }
        ScopeMode::All => unreachable!(),
    };

    let changed_files = paths.into_iter().collect::<Vec<_>>();
    let (mut components, unmatched_files) = project
        .config
        .classify_paths_with(&project.scope_rules, &changed_files);
    match project.config.scope.unmatched {
        UnmatchedScope::Fail if !unmatched_files.is_empty() => {
            return Err(ScopeError::UnmatchedFiles {
                count: unmatched_files.len(),
                files: unmatched_files.join(", "),
            });
        }
        UnmatchedScope::All => components.extend(project.config.components()),
        UnmatchedScope::Fail | UnmatchedScope::Ignore => {}
    }
    Ok(ScopeResult {
        mode: mode_label,
        components,
        changed_files,
        unmatched_files,
    })
}

fn ensure_git_worktree(project: &Project) -> Result<()> {
    let output = git::capture(&project.root, ["rev-parse", "--is-inside-work-tree"])?;
    if !output.status.success() || output.stdout != b"true\n" {
        bail!("project root is not a Git worktree");
    }
    Ok(())
}
