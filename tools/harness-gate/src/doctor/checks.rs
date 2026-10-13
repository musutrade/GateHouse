use super::report::DoctorReport;
use crate::config::{DoctorCheck, DoctorCheckKind, PathScope, PathType};
use crate::project::Project;
use anyhow::{bail, Context, Result};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub fn run(project: &Project) -> Result<super::DoctorReport> {
    let mut report = DoctorReport::new(project);
    for check in &project.config.doctor.checks {
        match run_check(project, check) {
            Ok(detail) => report.record_pass(&check.label, detail),
            Err(error) => {
                let detail = match &check.help {
                    Some(help) => format!("{error:#}; {help}"),
                    None => format!("{error:#}"),
                };
                report.record_failure(check.required, &check.label, detail);
            }
        }
    }
    Ok(report)
}

fn run_check(project: &Project, check: &DoctorCheck) -> Result<String> {
    let timeout = Duration::from_secs(check.timeout_secs);
    match &check.kind {
        DoctorCheckKind::Command { program, args } => {
            let args = args
                .iter()
                .map(|arg| project.expand(arg))
                .collect::<Vec<_>>();
            command_output(project, program, &args, timeout)
        }
        DoctorCheckKind::Path {
            path,
            path_type,
            path_scope,
        } => check_path(project, path, path_type, *path_scope),
        DoctorCheckKind::Glob { pattern } => check_glob(project, pattern),
        DoctorCheckKind::Env { name } => check_env(name),
        DoctorCheckKind::EnvOrFile {
            env,
            path,
            contains,
            path_scope,
        } => check_env_or_file(project, env, path, contains, *path_scope),
        DoctorCheckKind::GitConfig { key, expected } => {
            check_git_config(project, key, expected, timeout)
        }
        DoctorCheckKind::GitRemotes => check_remotes(&project.root, timeout),
        DoctorCheckKind::Version {
            program,
            args,
            path,
            trim_prefix,
            path_scope,
        } => check_version(
            project,
            program,
            args,
            path,
            trim_prefix,
            *path_scope,
            timeout,
        ),
        DoctorCheckKind::Service { service } => {
            crate::service::check_available(project, service, timeout)
        }
    }
}

pub(crate) fn check_path(
    project: &Project,
    path: &str,
    path_type: &PathType,
    scope: PathScope,
) -> Result<String> {
    let path = resolve_check_path(project, path, scope)?;
    let exists = match path_type {
        PathType::Any => path.exists(),
        PathType::File => path.is_file(),
        PathType::Directory => path.is_dir(),
    };
    if !exists {
        bail!("{} is missing", path.display());
    }
    Ok(path.display().to_string())
}

fn check_env(name: &str) -> Result<String> {
    std::env::var_os(name).ok_or_else(|| anyhow::anyhow!("{name} is not configured"))?;
    Ok(format!("{name} is configured"))
}

pub(crate) fn check_env_or_file(
    project: &Project,
    env: &str,
    path: &str,
    contains: &str,
    scope: PathScope,
) -> Result<String> {
    if std::env::var_os(env).is_some() {
        return Ok(format!("{env} is configured"));
    }
    let path = resolve_check_path(project, path, scope)?;
    let found = fs::read_to_string(&path)
        .map(|content| {
            content
                .lines()
                .any(|line| line.trim_start().starts_with(contains))
        })
        .unwrap_or(false);
    if !found {
        bail!(
            "{env} is absent and {} does not define {contains}",
            path.display()
        );
    }
    Ok(format!("{contains} found in {}", path.display()))
}

fn check_git_config(
    project: &Project,
    key: &str,
    expected: &str,
    timeout: Duration,
) -> Result<String> {
    let args = vec!["config".into(), "--get".into(), key.to_string()];
    let output = crate::process::capture("git", &args, &project.root, timeout)
        .with_context(|| format!("read Git config {key}"))?;
    if !output.status.success() {
        bail!("Git config {key} is not set");
    }
    let actual = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if actual != expected {
        bail!("Git config {key} is {actual:?}, expected {expected:?}");
    }
    Ok(format!("{key}={expected}"))
}

fn check_version(
    project: &Project,
    program: &str,
    args: &[String],
    path: &str,
    trim_prefix: &str,
    scope: PathScope,
    timeout: Duration,
) -> Result<String> {
    let args = args
        .iter()
        .map(|arg| project.expand(arg))
        .collect::<Vec<_>>();
    let actual = command_output(project, program, &args, timeout)?
        .trim_start_matches(trim_prefix)
        .to_string();
    let path = resolve_check_path(project, path, scope)?;
    let expected = fs::read_to_string(&path)
        .with_context(|| format!("read version file {}", path.display()))?
        .trim()
        .to_string();
    if actual != expected {
        bail!("found {actual}, expected {expected}");
    }
    Ok(expected)
}

fn resolve_check_path(project: &Project, value: &str, scope: PathScope) -> Result<PathBuf> {
    let mut expanded = project.expand(value);
    if cfg!(windows) {
        // `{root}` expands to a verbatim (extended-length) path on Windows, where `/`
        // is not a separator; normalize so `{root}/file` names a child of root.
        expanded = expanded.replace('/', "\\");
    }
    let path = Path::new(&expanded);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project.root.join(path)
    };
    if scope == PathScope::Repository {
        ensure_inside_repository(&project.root, &path).with_context(|| {
            format!("{value:?} is outside the repository; set path_scope = \"host\" to probe a host location")
        })?;
    }
    Ok(path)
}

/// Lexical and symlink-resolved containment for repository-scoped checks. A
/// missing target is judged by its nearest existing ancestor.
fn ensure_inside_repository(root: &Path, path: &Path) -> Result<()> {
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        bail!("path traverses upward: {}", path.display());
    }
    let root = root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", root.display()))?;
    let mut existing = path;
    while fs::symlink_metadata(existing).is_err() {
        existing = existing
            .parent()
            .ok_or_else(|| anyhow::anyhow!("cannot resolve {}", path.display()))?;
    }
    let resolved = existing
        .canonicalize()
        .with_context(|| format!("resolve {}", existing.display()))?;
    if !resolved.starts_with(&root) {
        bail!("path resolves outside the repository: {}", path.display());
    }
    Ok(())
}

fn command_output(
    project: &Project,
    program: &str,
    args: &[String],
    timeout: Duration,
) -> Result<String> {
    let output = crate::process::capture(program, args, &project.root, timeout)
        .with_context(|| format!("command {program} is not available or did not finish"))?;
    if !output.status.success() {
        bail!(
            "command {program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::to_string)
        .filter(|line| !line.is_empty())
        .ok_or_else(|| anyhow::anyhow!("command {program} produced no output"))
}

pub(crate) fn check_glob(project: &Project, pattern: &str) -> Result<String> {
    let pattern = project.expand(pattern);
    let absolute_pattern = Path::new(&pattern).is_absolute();
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(&pattern)?);
    let matcher = builder.build()?;
    let found = WalkBuilder::new(&project.root)
        .hidden(false)
        .build()
        .filter_map(Result::ok)
        .any(|entry| {
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                return false;
            }
            let candidate = if absolute_pattern {
                entry.path().to_path_buf()
            } else {
                entry
                    .path()
                    .strip_prefix(&project.root)
                    .unwrap_or(entry.path())
                    .to_path_buf()
            };
            matcher.is_match(candidate)
        });
    if !found {
        bail!("no files match {pattern}");
    }
    Ok(format!("matched {pattern}"))
}

pub(crate) fn check_remotes(project_root: &Path, timeout: Duration) -> Result<String> {
    let deadline = Instant::now() + timeout;
    let remotes = crate::process::capture(
        "git",
        &["remote".to_string()],
        project_root,
        remaining(deadline)?,
    )
    .context("list Git remotes")?;
    if !remotes.status.success() {
        bail!(
            "project root is not a Git worktree: {}",
            String::from_utf8_lossy(&remotes.stderr).trim()
        );
    }
    for remote in String::from_utf8_lossy(&remotes.stdout).lines() {
        let args = vec![
            "remote".into(),
            "get-url".into(),
            "--all".into(),
            remote.into(),
        ];
        let urls = crate::process::capture("git", &args, project_root, remaining(deadline)?)?;
        if !urls.status.success() {
            bail!("cannot read URLs for Git remote {remote:?}");
        }
        let unsafe_url = String::from_utf8_lossy(&urls.stdout).lines().any(|url| {
            (url.starts_with("https://") || url.starts_with("http://"))
                && url.split_once("//").is_some_and(|(_, tail)| {
                    tail.split_once('@')
                        .is_some_and(|(credentials, _)| !credentials.is_empty())
                })
        });
        if unsafe_url {
            bail!("remote {remote:?} contains embedded HTTPS credentials");
        }
    }
    Ok("no embedded credentials".into())
}

fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| anyhow::anyhow!("doctor check timed out"))
}
