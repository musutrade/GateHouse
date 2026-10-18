//! Conservative content identity; neither HEAD nor index object IDs stand in for bytes.
use crate::config::validate_cache_path as relative_file;
use crate::{
    config::{StepConfig, StepInput},
    project::Project,
};
use anyhow::{ensure, Context, Result};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

const MAX_INPUT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 100_000;

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn checked_file(path: &Path) -> Result<fs::File> {
    // Reject links in every component, including external toolchain paths.
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        current.push(component);
        ensure!(
            !fs::symlink_metadata(&current)?.file_type().is_symlink(),
            "cache input contains a link"
        );
    }
    ensure!(
        fs::symlink_metadata(path)?.is_file(),
        "cache input is not a regular file"
    );
    use std::os::unix::fs::OpenOptionsExt;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "cache input is not a regular file"
    );
    Ok(file)
}

fn hash_file(path: &Path, budget: &mut u64) -> Result<String> {
    let file = checked_file(path)?;
    let size = file.metadata()?.len();
    ensure!(size <= *budget, "cache input byte limit");
    let mut hasher = Sha256::new();
    let mut reader = file.take(size.saturating_add(1));
    let copied = std::io::copy(&mut reader, &mut hasher)?;
    ensure!(copied == size, "cache input changed while reading");
    *budget -= size;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        hasher.update(fs::metadata(path)?.permissions().mode().to_le_bytes());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn walk(root: &Path, relative: &Path, paths: &mut BTreeSet<String>) -> Result<()> {
    ensure!(
        relative.components().count() < 64,
        "cache snapshot depth limit"
    );
    for item in fs::read_dir(root.join(relative))? {
        let item = item?;
        let path = relative.join(item.file_name());
        ensure!(paths.len() < MAX_FILES, "cache file limit");
        let kind = item.file_type()?;
        ensure!(!kind.is_symlink(), "snapshot contains a link");
        if kind.is_dir() {
            walk(root, &path, paths)?;
        } else {
            paths.insert(
                path.to_str()
                    .context("non UTF-8 cache path")?
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

pub(super) fn outputs(project: &Project, step: &StepConfig) -> Result<BTreeMap<String, String>> {
    let cache = step.cache.as_ref().context("cache declaration absent")?;
    let mut budget = MAX_INPUT_BYTES;
    cache
        .outputs
        .iter()
        .map(|name| {
            relative_file(name)?;
            Ok((
                name.clone(),
                hash_file(&project.execution_root.join(name), &mut budget)?,
            ))
        })
        .collect()
}

pub(super) fn build(project: &Project, step: &StepConfig) -> Result<Vec<u8>> {
    ensure!(
        project.input().content.is_none(),
        "directory input executes without Git-based step reuse"
    );
    ensure!(
        step.input == StepInput::Snapshot,
        "repository-bound commands cannot be cached"
    );
    let cache = step.cache.as_ref().context("cache declaration absent")?;
    let mut budget = MAX_INPUT_BYTES;
    let source = source_identity(project, cache, &mut budget)?;
    let dependencies = dependency_outputs(project, step)?;
    let tools = tool_inputs(cache, &mut budget)?;
    let environment = environment_inputs(step, cache)?;
    let executable = core_digest()?;
    // This sensitive identity exists only in memory. Store.key signs it with a private host key.
    Ok(serde_json::to_vec(&json!({
        "version": "step-cache/v1", "core": env!("CARGO_PKG_VERSION"), "core_digest": executable,
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "project": project.input().project_identity, "mode": project.input().mode.as_str(),
        "profile": project.cache_profile, "configuration": project.config,
        "step": step, "source": source, "dependencies": dependencies, "tools": tools,
        "effective_cwd": project.expand_for_input(&step.cwd, step.input),
        "effective_args": step.args.iter().map(|a| project.expand_for_input(a, step.input)).collect::<Vec<_>>(),
        "head": project.input().commit,
        "environment": environment,
        "policies": policy_inputs(project, &mut budget)?,
    }))?)
}

fn policy_inputs(project: &Project, budget: &mut u64) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    // These are usually already source files. Bind them explicitly even when ignored.
    for path in [&project.audit_config, &project.secrets_config] {
        values.insert(
            path.strip_prefix(&project.execution_root)?
                .to_string_lossy()
                .into_owned(),
            hash_file(path, budget)?,
        );
    }
    for name in [
        ".harness-gate/quality.toml",
        ".harness-gate/plugins.lock.json",
    ] {
        let path = project.execution_root.join(name);
        if path.try_exists()? {
            values.insert(name.into(), hash_file(&path, budget)?);
        }
    }
    if let Some(config) = crate::config::quality::QualityConfig::load_optional(
        &project.execution_root,
        &project.config,
    )? {
        for binding in config.policies.values() {
            relative_file(&binding.policy_file)?;
            values.insert(
                binding.policy_file.clone(),
                hash_file(&project.execution_root.join(&binding.policy_file), budget)?,
            );
        }
    }
    Ok(values)
}

fn core_digest() -> Result<&'static str> {
    static DIGEST: std::sync::OnceLock<Result<String, String>> = std::sync::OnceLock::new();
    match DIGEST.get_or_init(|| {
        hash_file(
            &std::env::current_exe().map_err(|e| e.to_string())?,
            &mut (512 * 1024 * 1024),
        )
        .map_err(|e| e.to_string())
    }) {
        Ok(value) => Ok(value),
        Err(error) => anyhow::bail!("cannot bind executing Core: {error}"),
    }
}

fn source_identity(
    project: &Project,
    cache: &crate::config::StepCacheConfig,
    budget: &mut u64,
) -> Result<BTreeMap<String, String>> {
    let mut paths = source_paths(project)?;
    exclude_outputs(project, &mut paths)?;
    for name in &cache.inputs {
        relative_file(name)?;
        ensure!(
            project.execution_root.join(name).try_exists()?,
            "declared cache input missing"
        );
        paths.insert(name.clone());
    }
    exclude_reports(project, &mut paths)?;
    hash_sources(project, paths, budget)
}

fn exclude_reports(project: &Project, paths: &mut BTreeSet<String>) -> Result<()> {
    let reports = project.config.paths.reports.trim_start_matches("./");
    let indexed =
        crate::utils::git::capture_readonly(&project.root, ["ls-files", "-z", "--", reports])?;
    ensure!(
        indexed.status.success() && indexed.stdout.is_empty(),
        "tracked report files cannot be excluded from source identity"
    );
    paths.retain(|name| !Path::new(name).starts_with(reports));
    Ok(())
}

fn source_paths(project: &Project) -> Result<BTreeSet<String>> {
    let mut paths = BTreeSet::new();
    if project.input().is_snapshot() {
        walk(&project.execution_root, Path::new(""), &mut paths)?;
    } else {
        let output = crate::utils::git::capture_readonly(
            &project.root,
            [
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ],
        )?;
        ensure!(output.status.success(), "cannot enumerate source snapshot");
        for name in output.stdout.split(|b| *b == 0).filter(|v| !v.is_empty()) {
            paths.insert(std::str::from_utf8(name)?.to_string());
        }
    }
    ensure!(paths.len() <= MAX_FILES, "cache file limit");
    Ok(paths)
}

fn exclude_outputs(project: &Project, paths: &mut BTreeSet<String>) -> Result<()> {
    // Generated outputs cannot double as source. Tracked outputs are deliberately uncacheable.
    let all_outputs = project
        .config
        .steps
        .iter()
        .filter_map(|s| s.cache.as_ref())
        .flat_map(|c| &c.outputs)
        .collect::<BTreeSet<_>>();
    for name in &all_outputs {
        if paths.contains(*name) {
            // Untracked output files may be excluded, but indexed source never is.
            let indexed = crate::utils::git::capture_readonly(
                &project.root,
                ["ls-files", "--error-unmatch", "--", name.as_str()],
            )?;
            ensure!(
                !indexed.status.success(),
                "cache outputs must not be tracked source"
            );
            paths.remove(*name);
        }
    }
    Ok(())
}

fn hash_sources(
    project: &Project,
    paths: BTreeSet<String>,
    budget: &mut u64,
) -> Result<BTreeMap<String, String>> {
    let mut source = BTreeMap::new();
    for name in paths {
        relative_file(&name)?;
        let path = project.execution_root.join(&name);
        let value = match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => "absent".into(),
            Err(e) => return Err(e.into()),
            Ok(_) => hash_file(&path, budget)?,
        };
        source.insert(name, value);
    }
    Ok(source)
}

fn dependency_outputs(
    project: &Project,
    step: &StepConfig,
) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let mut dependencies = BTreeMap::new();
    for id in &step.depends_on {
        let dependency = project
            .config
            .step(id)
            .context("unknown cache dependency")?;
        let declaration = dependency
            .cache
            .as_ref()
            .context("dependency outputs not declared")?;
        ensure!(
            !declaration.outputs.is_empty(),
            "dependency has no bound outputs"
        );
        dependencies.insert(id, outputs(project, dependency)?);
    }
    Ok(dependencies
        .into_iter()
        .map(|(k, v)| (k.clone(), v))
        .collect())
}

fn tool_inputs(
    cache: &crate::config::StepCacheConfig,
    budget: &mut u64,
) -> Result<BTreeMap<String, String>> {
    let mut tools = BTreeMap::new();
    for tool in &cache.toolchain {
        ensure!(
            Path::new(tool).is_absolute(),
            "cache executable/toolchain must use absolute files"
        );
        tools.insert(tool, hash_file(Path::new(tool), budget)?);
    }
    Ok(tools.into_iter().map(|(k, v)| (k.clone(), v)).collect())
}

fn environment_inputs(
    step: &StepConfig,
    cache: &crate::config::StepCacheConfig,
) -> Result<BTreeMap<String, Option<String>>> {
    let environment = cache
        .environment
        .iter()
        .filter(|name| !step.remove_env.contains(name))
        .map(|name| match std::env::var(name) {
            Ok(value) => Ok((name, Some(value))),
            Err(std::env::VarError::NotPresent) => Ok((name, None)),
            Err(error) => Err(error.into()),
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    Ok(environment
        .into_iter()
        .map(|(k, v)| (k.clone(), v))
        .collect())
}
