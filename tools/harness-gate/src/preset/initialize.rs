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

/// Terminal-only, numbered preset selection; existing initialization owns all writes.
pub fn interactive(target: &Path, force: bool, directory: Option<&Path>) -> Result<Option<String>> {
    use std::io::{self, IsTerminal};
    anyhow::ensure!(io::stdin().is_terminal() && io::stderr().is_terminal(),
        "interactive initialization requires terminal input and stderr; use init --preset NAME for automation");
    let choices = catalog::choices(directory)?;
    select(
        &choices,
        target,
        force,
        &mut io::stdin().lock(),
        &mut io::stderr().lock(),
    )
}

fn answer(reader: &mut impl std::io::BufRead) -> Result<String> {
    use std::io::{BufRead, Read};
    let mut answer = String::new();
    reader.take(128).read_line(&mut answer)?;
    anyhow::ensure!(answer.len() < 128, "initialization response is too long");
    Ok(answer.trim().to_owned())
}

fn select(
    choices: &[(String, String)],
    target: &Path,
    force: bool,
    reader: &mut impl std::io::BufRead,
    output: &mut impl std::io::Write,
) -> Result<Option<String>> {
    for (index, (name, description)) in choices.iter().enumerate() {
        writeln!(
            output,
            "{}: {} — {}",
            index + 1,
            name.escape_debug(),
            description.escape_debug()
        )?;
    }
    write!(output, "Preset number (empty or q cancels): ")?;
    output.flush()?;
    let value = answer(reader)?;
    if value.is_empty() || value == "q" {
        return Ok(None);
    }
    let selected = value
        .parse::<usize>()
        .ok()
        .and_then(|number| number.checked_sub(1))
        .and_then(|index| choices.get(index))
        .context("invalid preset number; no configuration written")?;
    writeln!(
        output,
        "Initialize {:?} in {} (replace existing files: {})?",
        selected.0,
        target.display().to_string().escape_debug(),
        force
    )?;
    write!(output, "Confirm [y/N]: ")?;
    output.flush()?;
    if !matches!(answer(reader)?.as_str(), "y" | "Y" | "yes" | "YES") {
        return Ok(None);
    }
    Ok(Some(selected.0.clone()))
}

#[cfg(test)]
mod interactive_tests {
    use super::*;
    #[test]
    fn selected_external_catalog_is_validated_and_initialized() {
        let catalog_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            catalog_dir.path().join("flow.toml"),
            include_str!("../../presets/generic.flow.toml"),
        )
        .unwrap();
        std::fs::write(catalog_dir.path().join("catalog.json"), r#"{"schema":"harness-preset-catalog/v1","presets":{"custom":{"description":"Reviewed external preset","flow":"flow.toml"}},"packs":{}}"#).unwrap();
        let choices = catalog::choices(Some(catalog_dir.path())).unwrap();
        assert_eq!(choices.len(), 2);
        let root = tempfile::tempdir().unwrap();
        let name = select(
            &choices,
            root.path(),
            false,
            &mut "2\ny\n".as_bytes(),
            &mut Vec::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(name, "custom");
        init_from(root.path(), &name, false, Some(catalog_dir.path())).unwrap();
        FlowConfig::from_source(
            &std::fs::read_to_string(root.path().join(DEFAULT_CONFIG_PATH)).unwrap(),
        )
        .unwrap()
        .validate()
        .unwrap();
        std::fs::write(catalog_dir.path().join("catalog.json"), "{}").unwrap();
        assert!(catalog::choices(Some(catalog_dir.path())).is_err());
    }

    #[test]
    fn selection_requires_number_and_explicit_confirmation() {
        let choices = vec![("generic".into(), "safe\x1b[2J".into())];
        for input in ["", "\n", "q\n", "1\n", "1\nn\n"] {
            assert!(select(
                &choices,
                Path::new("project"),
                false,
                &mut input.as_bytes(),
                &mut Vec::new()
            )
            .unwrap()
            .is_none());
        }
        for input in ["0\ny\n", "2\ny\n", "text\ny\n"] {
            assert!(select(
                &choices,
                Path::new("project"),
                false,
                &mut input.as_bytes(),
                &mut Vec::new()
            )
            .is_err());
        }
        let mut output = Vec::new();
        assert_eq!(
            select(
                &choices,
                Path::new("project"),
                false,
                &mut "1\ny\n".as_bytes(),
                &mut output
            )
            .unwrap()
            .as_deref(),
            Some("generic")
        );
        assert!(!output.contains(&27));
        assert!(answer(&mut "x".repeat(128).as_bytes()).is_err());
    }
}
