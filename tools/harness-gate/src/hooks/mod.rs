//! Explicit lifecycle for exact, thin repository hooks. No gate execution here.
mod filesystem;
mod paths;

use anyhow::{ensure, Context, Result};
use filesystem::State;
use std::path::Path;

#[derive(Debug, Clone, Copy, clap::Subcommand)]
pub(crate) enum HookAction {
    /// Install both thin hooks without overwriting existing files.
    Install,
    /// Inspect both hooks without changing files (0 only when both are current).
    Status,
    /// Remove only exact managed hooks; absent hooks are harmless.
    Uninstall,
}

const HOOKS: [(&str, &str); 2] = [
    (
        "pre-commit",
        "#!/bin/sh\n# harness-gate managed hook v1\nexec harness-gate hook\n",
    ),
    (
        "pre-push",
        "#!/bin/sh\n# harness-gate managed hook v1\nexec harness-gate verify --all\n",
    ),
];

fn readiness(root: &Path) -> Result<()> {
    let project = crate::project::Project::discover_for_plan(Some(root.to_path_buf()), None)?;
    let profile = &project.config.project.hook_profile;
    ensure!(
        project
            .config
            .steps
            .iter()
            .any(|step| step.profiles.contains(profile)),
        "configured hook profile {profile:?} has no participating steps"
    );
    Ok(())
}

pub(crate) fn run(root: &Path, action: HookAction) -> Result<bool> {
    let directory = paths::resolve(root)?;
    if matches!(action, HookAction::Status)
        && directory
            .join(".harness-gate-hooks.lock")
            .symlink_metadata()
            .is_ok()
    {
        println!("Hooks lifecycle: busy (lock exists)");
        return Ok(false);
    }
    let _lock = if matches!(action, HookAction::Status) {
        None
    } else {
        Some(filesystem::Lock::acquire(&directory)?)
    };
    let states = HOOKS
        .iter()
        .map(|(name, content)| filesystem::inspect(&directory.join(name), content.as_bytes()))
        .collect::<Result<Vec<_>>>()?;
    if matches!(action, HookAction::Status) {
        return status(root, &states);
    }
    ensure!(
        states
            .iter()
            .all(|state| matches!(state, State::Absent | State::Current | State::NotExecutable)),
        "existing hooks are modified, foreign or unsafe; no files changed"
    );
    if matches!(action, HookAction::Uninstall) {
        uninstall(&directory, &states)
            .context("uninstall incomplete; inspect hooks status before retrying")?;
        println!("Managed hooks removed (already absent is unchanged).");
        return Ok(true);
    }
    readiness(root)?;
    ensure!(
        !states.contains(&State::NotExecutable),
        "managed hook is not executable; restore its permissions explicitly"
    );
    install(&directory, &states)?;
    println!("Managed pre-commit and pre-push hooks installed. No checks were executed.");
    Ok(true)
}

fn status(root: &Path, states: &[State]) -> Result<bool> {
    for ((name, _), state) in HOOKS.iter().zip(states) {
        println!("{name}: {state:?}");
    }
    match readiness(root) {
        Ok(()) => println!("Configuration: hook profile participates"),
        Err(error) => println!("Configuration: {}", error.to_string().escape_debug()),
    }
    Ok(states.iter().all(|state| *state == State::Current))
}

fn uninstall(directory: &Path, states: &[State]) -> Result<()> {
    let identities = HOOKS
        .iter()
        .zip(states)
        .map(|((name, _), state)| {
            if *state == State::Absent {
                Ok(None)
            } else {
                std::fs::symlink_metadata(directory.join(name)).map(Some)
            }
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    for ((name, content), identity) in HOOKS.iter().zip(identities) {
        if let Some(identity) = identity {
            filesystem::remove_owned(&directory.join(name), content.as_bytes(), &identity)?;
        }
    }
    Ok(())
}

fn install(directory: &Path, states: &[State]) -> Result<()> {
    let mut created = Vec::new();
    for ((name, content), state) in HOOKS.iter().zip(states) {
        if *state == State::Current {
            continue;
        }
        match filesystem::publish(&directory.join(name), content.as_bytes()) {
            Ok(identity) => created.push((*name, content.as_bytes(), identity)),
            Err(error) => {
                for (name, content, identity) in created {
                    if let Err(cleanup) =
                        filesystem::remove_owned(&directory.join(name), content, &identity)
                    {
                        return Err(error.context(format!(
                            "rollback retained changed hook: {cleanup}; inspect hooks status"
                        )));
                    }
                }
                return Err(error.context(
                    "installation stopped; hooks created by this operation were rolled back",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_second_target_conflict_rolls_back_only_created_hook() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("pre-push"),
            "foreign arriving after preflight",
        )
        .unwrap();
        assert!(install(root.path(), &[State::Absent, State::Absent]).is_err());
        assert!(!root.path().join("pre-commit").exists());
        assert_eq!(
            std::fs::read(root.path().join("pre-push")).unwrap(),
            b"foreign arriving after preflight"
        );
    }
}
