use super::{resolve_repo_path, InvocationInput, Project};
use crate::config::{
    resolve_config_path, ConfigCode, ConfigDiagnostics, FlowConfig, DEFAULT_CONFIG_PATH,
};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

impl Project {
    pub fn discover(
        override_root: Option<PathBuf>,
        config_override: Option<PathBuf>,
    ) -> Result<Self> {
        Self::discover_with_access(override_root, config_override, false)
    }

    pub(crate) fn discover_for_plan(
        override_root: Option<PathBuf>,
        config_override: Option<PathBuf>,
    ) -> Result<Self> {
        Self::discover_with_access(override_root, config_override, true)
    }

    /// Discover the index adapter without parsing or trusting the working-tree
    /// workflow. Unstaged configuration edits must not change the selected input.
    pub(crate) fn discover_index_content(
        override_root: Option<PathBuf>,
        config_override: Option<PathBuf>,
        destination: Option<&Path>,
    ) -> Result<Self> {
        let start = canonical_directory(&override_root.clone().unwrap_or(env::current_dir()?))?;
        let top = crate::utils::git::capture_readonly(&start, ["rev-parse", "--show-toplevel"])?;
        anyhow::ensure!(
            top.status.success(),
            "index snapshot requires a Git repository"
        );
        let root = canonical_directory(Path::new(
            std::str::from_utf8(&top.stdout)?.trim_end_matches(['\r', '\n']),
        ))?;
        anyhow::ensure!(
            override_root.is_none() || root == start,
            "index snapshot project root must be the Git repository root"
        );
        let config = config_override.unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
        let relative = if config.is_absolute() {
            config.strip_prefix(&root)?.to_path_buf()
        } else {
            config
        };
        crate::utils::git::index_path(&relative)?;
        let input = InvocationInput::materialize_staged(&root, &relative, "not-sampled")?;
        let indexed = Self::from_input(root, &relative, input)?;
        let mut snapshot = indexed.directory_snapshot(destination)?;
        snapshot.invocation_input.mode = super::InputMode::ContentIndex;
        Ok(snapshot)
    }

    fn discover_with_access(
        override_root: Option<PathBuf>,
        config_override: Option<PathBuf>,
        readonly: bool,
    ) -> Result<Self> {
        let root = match override_root {
            Some(root) => canonical_directory(&root)?,
            None => {
                let start = env::current_dir().context("read current directory")?;
                find_root(&start, config_override.as_deref())?
            }
        };
        let config_path = resolve_config_path(&root, config_override)?;
        let config_relative = config_path
            .strip_prefix(&root)
            .with_context(|| {
                format!(
                    "workflow config must be inside the repository: {}",
                    config_path.display()
                )
            })?
            .to_path_buf();
        let config_bytes = fs::read(&config_path)
            .with_context(|| format!("read workflow config {}", config_path.display()))?;
        let input = InvocationInput::working_tree(&root, &config_bytes, readonly)?;
        Self::from_input(root, &config_relative, input)
    }

    pub(crate) fn directory_snapshot(&self, destination: Option<&Path>) -> Result<Self> {
        let relative = self.config_path.strip_prefix(&self.execution_root)?;
        let output = self.reports.strip_prefix(&self.root)?;
        let input = InvocationInput::directory_snapshot(&self.execution_root, output, destination)?;
        let mut project = Self::from_input(self.root.clone(), relative, input)?;
        anyhow::ensure!(
            project
                .config
                .steps
                .iter()
                .all(|step| step.input != crate::config::StepInput::Repository),
            "directory input does not support repository-bound steps"
        );
        project.replay_state_dir = self.replay_state_dir.clone();
        project.step_cache_dir = self.step_cache_dir.clone();
        project.plugins = self.plugins.clone();
        Ok(project)
    }

    pub(crate) fn staged_snapshot(&self) -> Result<Self> {
        let config_relative = self
            .config_path
            .strip_prefix(&self.execution_root)
            .with_context(|| {
                format!(
                    "workflow config must be inside the invocation input: {}",
                    self.config_path.display()
                )
            })?;
        let input =
            InvocationInput::materialize_staged(&self.root, config_relative, &self.input().commit)?;
        let mut snapshot = Self::from_input(self.root.clone(), config_relative, input)?;
        snapshot.replay_state_dir = self.replay_state_dir.clone();
        snapshot.step_cache_dir = self.step_cache_dir.clone();
        snapshot.plugins = self.plugins.clone();
        Ok(snapshot)
    }

    /// Inspect staged configuration using only read-only Git commands. No
    /// write-tree, checkout filters, report allocation or repository writes.
    pub(crate) fn staged_plan_snapshot(&self) -> Result<Self> {
        let relative = self.config_path.strip_prefix(&self.execution_root)?;
        let input = InvocationInput::inspect_staged(&self.root, relative, &self.input().commit)?;
        let mut snapshot = Self::from_input(self.root.clone(), relative, input)?;
        snapshot.plugins = self.plugins.clone();
        Ok(snapshot)
    }

    fn from_input(root: PathBuf, config_relative: &Path, input: InvocationInput) -> Result<Self> {
        let execution_root = input.execution_root.clone();
        let config_path = execution_root.join(config_relative);
        let config = FlowConfig::load_with_diagnostics(&config_path, Some(&execution_root))
            .map_err(anyhow::Error::from)?;
        let mut input = input;
        input.configuration_digest = configuration_digest(&config)?;
        let scope_rules = config.compile_scope_rules()?;
        let reports = resolve_repo_path(
            &root,
            Path::new(&config.paths.reports),
            "report directory",
            false,
        )
        .map_err(|_| {
            anyhow::Error::new(
                ConfigDiagnostics::single(
                    ConfigCode::HgcfgInvalidPath,
                    "paths.reports",
                    "report directory path could not be resolved safely",
                    "use a repository-relative report directory without symlink escape",
                )
                .with_source(config_path.clone()),
            )
        })?;
        let audit_config = resolve_repo_path(
            &execution_root,
            Path::new(&config.paths.audit_config),
            "audit configuration",
            true,
        )
        .map_err(|_| {
            anyhow::Error::new(
                ConfigDiagnostics::single(
                    ConfigCode::HgcfgInvalidPath,
                    "paths.audit_config",
                    "audit configuration path could not be resolved safely",
                    "use a repository-relative audit configuration path inside the repository",
                )
                .with_source(config_path.clone()),
            )
        })?;
        let secrets_config = resolve_repo_path(
            &execution_root,
            Path::new(&config.paths.secrets_config),
            "secret scan configuration",
            true,
        )
        .map_err(|_| {
            anyhow::Error::new(
                ConfigDiagnostics::single(
                    ConfigCode::HgcfgInvalidPath,
                    "paths.secrets_config",
                    "secret scan configuration path could not be resolved safely",
                    "use a repository-relative secret configuration path inside the repository",
                )
                .with_source(config_path.clone()),
            )
        })?;
        let aliases = config
            .paths
            .aliases
            .iter()
            .map(|(name, entry)| {
                resolve_repo_path(
                    &execution_root,
                    Path::new(&entry.path),
                    &format!("path alias {name:?}"),
                    false,
                )
                .map(|path| (name.clone(), path))
                .map_err(|_| {
                    anyhow::Error::new(
                        ConfigDiagnostics::single(
                            ConfigCode::HgcfgInvalidPath,
                            format!("paths.aliases[\"{name}\"]"),
                            "path alias could not be resolved safely",
                            "use a repository-relative alias path without symlink escape",
                        )
                        .with_source(config_path.clone()),
                    )
                })
            })
            .collect::<Result<BTreeMap<_, _>>>()?;

        let project = Self {
            root,
            replay_state_dir: None,
            step_cache_dir: None,
            cache_profile: None,
            plugins: crate::plugins::HostOptions::default(),
            execution_root,
            invocation_input: input,
            config_path,
            config,
            reports: reports.clone(),
            audit_config,
            secrets_config,
            resource_leases: reports.join("leases"),
            aliases,
            scope_rules,
        };
        project.validate()?;
        Ok(project)
    }

    fn validate(&self) -> Result<()> {
        if !self.audit_config.is_file() {
            return Err(anyhow::Error::new(
                ConfigDiagnostics::single(
                    ConfigCode::HgcfgRequiredFile,
                    "paths.audit_config",
                    "required audit configuration file is missing",
                    "create the configured audit configuration file or update paths.audit_config",
                )
                .with_source(self.audit_config.clone()),
            ));
        }
        if !self.secrets_config.is_file() {
            return Err(anyhow::Error::new(
                ConfigDiagnostics::single(
                    ConfigCode::HgcfgRequiredFile,
                    "paths.secrets_config",
                    "required secret scan configuration file is missing",
                    "create the configured secret scan configuration file or update paths.secrets_config",
                )
                .with_source(self.secrets_config.clone()),
            ));
        }
        Ok(())
    }
}

fn configuration_digest(config: &FlowConfig) -> Result<String> {
    use sha2::{Digest, Sha256};
    let canonical =
        toml::to_string(config).context("serialize effective workflow configuration")?;
    let mut digest = Sha256::new();
    digest.update(canonical.as_bytes());
    Ok(format!("sha256:{:x}", digest.finalize()))
}

fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("resolve project path {}", path.display()))?;
    if !path.is_dir() {
        bail!("project root is not a directory: {}", path.display());
    }
    Ok(path)
}

fn find_root(start: &Path, config_override: Option<&Path>) -> Result<PathBuf> {
    let start = canonical_directory(start)?;
    let configured_path = config_override.map(Path::to_path_buf);
    if let Some(config) = configured_path {
        let candidate = if config.is_absolute() {
            config
        } else {
            start.join(config)
        };
        let config = candidate
            .canonicalize()
            .with_context(|| format!("resolve workflow config {}", candidate.display()))?;
        if let Some(root) = config.ancestors().find(|path| path.join(".git").exists()) {
            return Ok(root.to_path_buf());
        }
    }
    start
        .ancestors()
        .find(|candidate| candidate.join(DEFAULT_CONFIG_PATH).is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "could not find project root above {}; expected {}; run `harness-gate init --preset <name>`",
                start.display(),
                DEFAULT_CONFIG_PATH
            )
        })
}
