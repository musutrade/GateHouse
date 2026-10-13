use super::catalog::{self, AUDIT_TEMPLATE, GITIGNORE_TEMPLATE, SECRETS_TEMPLATE};
use super::filesystem::{atomic_write_batch, ensure_writable, resolve_inside};
use crate::config::{FlowConfig, DEFAULT_CONFIG_PATH};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn init(target: &Path, name: &str, force: bool) -> Result<()> {
    init_from(target, name, force, None)
}

pub fn init_from(target: &Path, name: &str, force: bool, directory: Option<&Path>) -> Result<()> {
    let preset = catalog::find(name, directory)?;
    fs::create_dir_all(target)
        .with_context(|| format!("create project directory {}", target.display()))?;
    let root = target
        .canonicalize()
        .with_context(|| format!("resolve project directory {}", target.display()))?;
    let flow_path = resolve_inside(&root, PathBuf::from(DEFAULT_CONFIG_PATH))?;
    let audit_path = resolve_inside(&root, PathBuf::from(".harness-gate/audit.toml"))?;
    let secrets_path = resolve_inside(&root, PathBuf::from(".harness-gate/secrets.toml"))?;
    ensure_writable(&flow_path, force)?;
    ensure_writable(&audit_path, force)?;
    ensure_writable(&secrets_path, force)?;

    let mut config = FlowConfig::from_source(&preset.flow)?;
    config.project.name = project_id(&root);
    config.validate()?;
    let directory = flow_path.parent().context("flow config has no parent")?;
    let flow_content = toml::to_string_pretty(&config)?;
    let mut extra_files = Vec::new();
    if let Some(recipe) = &preset.recipe {
        let composition = super::composition::compose(
            recipe,
            &preset
                .packs
                .iter()
                .map(|(id, text)| (id.as_str(), text.as_str()))
                .collect::<Vec<_>>(),
            &config.project.name,
        )?;
        let mut files = composition.files;
        files.insert(
            crate::config::quality::QUALITY_CONFIG_PATH.into(),
            toml::to_string_pretty(&composition.quality)?,
        );
        files.insert(
            ".harness-gate/QUALITY.md".into(),
            include_str!("../../presets/QUALITY.md").into(),
        );
        for (path, content) in files {
            let path = resolve_inside(&root, PathBuf::from(path))?;
            ensure_writable(&path, force)?;
            extra_files.push((path, content));
        }
    }
    let gitignore = resolve_inside(&root, PathBuf::from(".harness-gate/.gitignore"))?;
    let mut flow_entries = vec![
        (audit_path.as_path(), AUDIT_TEMPLATE.as_bytes()),
        (secrets_path.as_path(), SECRETS_TEMPLATE.as_bytes()),
        (flow_path.as_path(), flow_content.as_bytes()),
    ];
    if fs::symlink_metadata(&gitignore).is_err() {
        flow_entries.push((gitignore.as_path(), GITIGNORE_TEMPLATE.as_bytes()));
    }
    flow_entries.extend(
        extra_files
            .iter()
            .map(|(path, content)| (path.as_path(), content.as_bytes())),
    );
    atomic_write_batch(&flow_entries)?;

    println!("Initialized preset {name:?} in {}", directory.display());
    println!(
        "Next: harness-gate --project-root {} config check",
        root.display()
    );
    if preset.recipe.is_some() {
        println!("Quality enabled: review .harness-gate/QUALITY.md and provision signed workflow inputs before verify.");
    } else {
        println!("Quality remains opt-in; no quality policy was generated.");
    }
    Ok(())
}

pub(super) fn project_id(root: &Path) -> String {
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    let mut id = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character.to_ascii_lowercase());
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_matches('-');
    if id.is_empty() {
        "project".into()
    } else if !id.starts_with(|c: char| c.is_ascii_alphabetic()) {
        format!("project-{id}")
    } else {
        id.into()
    }
}
