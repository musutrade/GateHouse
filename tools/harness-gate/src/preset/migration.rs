use super::catalog::SECRETS_TEMPLATE;
use super::filesystem::{atomic_write, ensure_writable, resolve_inside};
use super::initialize::project_id;
use crate::config::{migrate_v1, CONFIG_VERSION, DEFAULT_CONFIG_PATH};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn migrate(
    root: &Path,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", root.display()))?;
    // There is no conventional v1 location; guessing a historical path only
    // produced a confusing missing-file error (#321).
    let Some(input) = input else {
        bail!("config migrate requires --input <PATH> naming the schema v1 flow.toml (or the global --config)");
    };
    let input = resolve_inside(&root, input)?;
    let output = resolve_inside(
        &root,
        output.unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH)),
    )?;
    ensure_writable(&output, force)?;

    let source = fs::read_to_string(&input)
        .with_context(|| format!("read v1 workflow config {}", input.display()))?;
    let config = migrate_v1(&source, &project_id(&root))?;
    if config.version != CONFIG_VERSION {
        bail!("migration did not produce schema v{CONFIG_VERSION}");
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let secrets_path = resolve_inside(&root, PathBuf::from(".harness-gate/secrets.toml"))?;
    if !secrets_path.exists() {
        atomic_write(&secrets_path, SECRETS_TEMPLATE.as_bytes())?;
    }
    atomic_write(&output, toml::to_string_pretty(&config)?.as_bytes())?;
    println!("Migrated {} -> {}", input.display(), output.display());
    println!("The source file was not removed.");
    Ok(())
}
