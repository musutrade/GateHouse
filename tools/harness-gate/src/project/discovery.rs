use super::{resolve_repo_path, InvocationInput, Project};
use crate::config::{resolve_config_path, ConfigDiagnostics, FlowConfig, DEFAULT_CONFIG_PATH};
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
        let input = InvocationInput::working_tree(&root, &config_bytes)?;
        Self::from_input(root, &config_relative, input)
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
        let input = InvocationInput::materialize_staged(&self.root, config_relative)?;
        let mut snapshot = Self::from_input(self.root.clone(), config_relative, input)?;
        snapshot.replay_state_dir = self.replay_state_dir.clone();
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
                    "HGCFG-INVALID-PATH",
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
                    "HGCFG-INVALID-PATH",
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
                    "HGCFG-INVALID-PATH",
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
                            "HGCFG-INVALID-PATH",
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
                    "HGCFG-REQUIRED-FILE",
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
                    "HGCFG-REQUIRED-FILE",
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
