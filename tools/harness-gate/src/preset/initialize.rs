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
    initialize(target, name, force, directory, None)
}

pub fn init_named(
    target: &Path,
    name: &str,
    force: bool,
    directory: Option<&Path>,
    project_name: Option<&str>,
) -> Result<()> {
    match (directory, project_name) {
        (None, None) => init(target, name, force),
        (Some(directory), None) => init_from(target, name, force, Some(directory)),
        (_, Some(_)) => initialize(target, name, force, directory, project_name),
    }
}

fn initialize(
    target: &Path,
    name: &str,
    force: bool,
    directory: Option<&Path>,
    project_name: Option<&str>,
) -> Result<()> {
    if let Some(name) = project_name {
        validate_project_name(name)?;
    }
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
    config.project.name = project_name
        .map(str::to_owned)
        .unwrap_or_else(|| project_id(&root));
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
        crate::config::quality::validate_project_identifier(&composition.quality.project.id)?;
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

    println!(
        "Initialized preset {name:?} in {}",
        directory.display().to_string().escape_debug()
    );
    println!(
        "Project directory (display only): {}",
        root.display().to_string().escape_debug()
    );
    println!("From that project directory, run:");
    println!("  harness-gate config check");
    println!("    Validate the generated configuration.");
    println!("  harness-gate doctor");
    println!("    Execute the declared dependency checks.");
    println!("  harness-gate verify --plan --all");
    println!(
        "    Inspect the configured default-profile plan without running gates, commands or collectors."
    );
    if preset.recipe.is_some() {
        println!("Quality enabled: review .harness-gate/QUALITY.md and provision signed workflow inputs before verify.");
    } else {
        println!("Quality remains opt-in; no quality policy was generated.");
    }
    println!("After reviewing the configuration and prerequisites, run harness-gate verify --all to execute verification.");
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

fn validate_project_name(name: &str) -> Result<()> {
    let mut config = FlowConfig::from_source(include_str!("../../presets/generic.flow.toml"))?;
    config.project.name = name.into();
    config.validate()
}

pub struct InitSelection {
    pub preset: String,
    pub project_name: String,
}

/// Terminal-only, numbered preset selection; existing initialization owns all writes.
pub fn interactive(
    target: &Path,
    force: bool,
    directory: Option<&Path>,
    project_name: Option<&str>,
) -> Result<Option<InitSelection>> {
    use std::io::{self, IsTerminal};
    anyhow::ensure!(io::stdin().is_terminal() && io::stderr().is_terminal(),
        "interactive initialization requires terminal input and stderr; use init --preset NAME for automation");
    let default_name = project_name.map(str::to_owned).unwrap_or_else(|| {
        project_id(
            &target
                .canonicalize()
                .unwrap_or_else(|_| target.to_path_buf()),
        )
    });
    validate_project_name(&default_name)?;
    let menu = catalog::choices(directory, Some(target))?;
    select(
        &menu.choices,
        menu.recommendation.as_ref(),
        target,
        force,
        &default_name,
        &mut io::stdin().lock(),
        &mut io::stderr().lock(),
    )
}

fn answer(reader: &mut impl std::io::BufRead) -> Result<Option<String>> {
    read_answer(reader, 128)
}

fn read_answer(reader: &mut impl std::io::BufRead, limit: u64) -> Result<Option<String>> {
    use std::io::{BufRead, Read};
    let mut answer = String::new();
    let read = reader.take(limit).read_line(&mut answer)?;
    anyhow::ensure!(
        (answer.len() as u64) < limit,
        "initialization response is too long"
    );
    Ok((read != 0).then(|| answer.trim_end_matches(['\r', '\n']).to_owned()))
}

fn select(
    choices: &[(String, String)],
    recommendation: Option<&catalog::Recommendation>,
    target: &Path,
    force: bool,
    default_name: &str,
    reader: &mut impl std::io::BufRead,
    output: &mut impl std::io::Write,
) -> Result<Option<InitSelection>> {
    let Some(preset) = select_preset(choices, recommendation, reader, output)? else {
        return Ok(None);
    };
    let Some(project_name) = select_project_name(default_name, reader, output)? else {
        return Ok(None);
    };
    writeln!(
        output,
        "Project {}: initialize {:?} in {} (replace existing files: {})?",
        project_name.escape_debug(),
        preset,
        target.display().to_string().escape_debug(),
        force
    )?;
    write!(output, "Confirm [y/N]: ")?;
    output.flush()?;
    if !matches!(
        answer(reader)?.unwrap_or_default().trim(),
        "y" | "Y" | "yes" | "YES"
    ) {
        return Ok(None);
    }
    Ok(Some(InitSelection {
        preset,
        project_name,
    }))
}

fn select_preset(
    choices: &[(String, String)],
    recommendation: Option<&catalog::Recommendation>,
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
    if let Some(recommendation) = recommendation {
        writeln!(output, "Suggested preset: {} (catalog marker match: {}). Type d to select it; this is not a compatibility guarantee.", recommendation.name.escape_debug(), recommendation.markers.join(", "))?;
    }
    loop {
        write!(output, "Preset number (empty or q cancels): ")?;
        output.flush()?;
        let value = answer(reader)?.unwrap_or_default();
        let value = value.trim();
        if value.is_empty() || value == "q" {
            return Ok(None);
        }
        let selected = if value == "d" {
            recommendation.map(|suggestion| suggestion.name.as_str())
        } else {
            value
                .parse::<usize>()
                .ok()
                .and_then(|number| number.checked_sub(1))
                .and_then(|index| choices.get(index))
                .map(|(name, _)| name.as_str())
        };
        if let Some(name) = selected {
            return Ok(Some(name.to_owned()));
        }
        writeln!(output, "Invalid preset selection; choose a listed number or d only when a suggestion is shown.")?;
    }
}

fn select_project_name(
    default_name: &str,
    reader: &mut impl std::io::BufRead,
    output: &mut impl std::io::Write,
) -> Result<Option<String>> {
    loop {
        write!(
            output,
            "Project name [{}] (empty uses default, EOF cancels): ",
            default_name.escape_debug()
        )?;
        output.flush()?;
        let Some(value) = read_answer(reader, 4096)? else {
            return Ok(None);
        };
        let name = if value.is_empty() {
            default_name.to_owned()
        } else {
            value
        };
        if validate_project_name(&name).is_ok() {
            return Ok(Some(name));
        }
        writeln!(output, "Invalid project name; use a nonempty name containing only lowercase letters, digits, dots, underscores or hyphens.")?;
    }
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
        let choices = catalog::choices(Some(catalog_dir.path()), None)
            .unwrap()
            .choices;
        assert_eq!(choices.len(), 2);
        let root = tempfile::tempdir().unwrap();
        let name = select(
            &choices,
            None,
            root.path(),
            false,
            "project",
            &mut "2\ncustom-name\ny\n".as_bytes(),
            &mut Vec::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(name.preset, "custom");
        assert_eq!(name.project_name, "custom-name");
        init_named(
            root.path(),
            &name.preset,
            false,
            Some(catalog_dir.path()),
            Some(&name.project_name),
        )
        .unwrap();
        FlowConfig::from_source(
            &std::fs::read_to_string(root.path().join(DEFAULT_CONFIG_PATH)).unwrap(),
        )
        .unwrap()
        .validate()
        .unwrap();
        std::fs::write(catalog_dir.path().join("catalog.json"), "{}").unwrap();
        assert!(catalog::choices(Some(catalog_dir.path()), None).is_err());
    }

    #[test]
    fn project_name_prompt_distinguishes_empty_from_eof_and_preserves_valid_ids() {
        assert!(
            select_project_name("default", &mut "".as_bytes(), &mut Vec::new())
                .unwrap()
                .is_none()
        );
        for (input, expected) in [
            ("\n", "default"),
            ("q\n", "q"),
            ("7.valid_id-\n", "7.valid_id-"),
        ] {
            assert_eq!(
                select_project_name("default", &mut input.as_bytes(), &mut Vec::new())
                    .unwrap()
                    .as_deref(),
                Some(expected)
            );
        }
        for input in ["UPPER\n", " bad\n", "bad/name\n"] {
            assert!(
                select_project_name("default", &mut input.as_bytes(), &mut Vec::new())
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn retries_invalid_input_but_not_reader_failures() {
        let choices = vec![("generic".into(), "Default".into())];
        let mut output = Vec::new();
        assert_eq!(
            select_preset(
                &choices,
                None,
                &mut "0\nd\n999999999999999999999999\n1\n".as_bytes(),
                &mut output
            )
            .unwrap()
            .as_deref(),
            Some("generic")
        );
        for (input, expected) in [
            ("bad name\nq\n", Some("q")),
            ("BAD\n\n", Some("default")),
            ("BAD\n", None),
        ] {
            assert_eq!(
                select_project_name("default", &mut input.as_bytes(), &mut output)
                    .unwrap()
                    .as_deref(),
                expected
            );
        }
        assert!(select_preset(
            &choices,
            None,
            &mut format!("{}\n1\n", "x".repeat(128)).as_bytes(),
            &mut output
        )
        .is_err());
        assert!(select_project_name(
            "default",
            &mut format!("{}\nvalid\n", "x".repeat(4096)).as_bytes(),
            &mut output
        )
        .is_err());
        struct Broken;
        impl std::io::Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("injected input failure"))
            }
        }
        let mut broken = std::io::BufReader::new(Broken);
        assert!(select_preset(&choices, None, &mut broken, &mut output).is_err());
        let mut broken = std::io::BufReader::new(Broken);
        assert!(select_project_name("default", &mut broken, &mut output).is_err());
    }

    #[test]
    fn suggested_preset_requires_explicit_choice() {
        let choices = vec![
            ("generic".into(), "Default".into()),
            ("custom".into(), "Explicit".into()),
        ];
        let suggestion = catalog::Recommendation {
            name: "custom".into(),
            markers: vec!["Cargo.toml".into()],
        };
        let mut output = Vec::new();
        assert_eq!(
            select_preset(
                &choices,
                Some(&suggestion),
                &mut "d\n".as_bytes(),
                &mut output
            )
            .unwrap()
            .as_deref(),
            Some("custom")
        );
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("catalog marker match: Cargo.toml"));
        assert_eq!(
            select_preset(
                &choices,
                Some(&suggestion),
                &mut "1\n".as_bytes(),
                &mut Vec::new()
            )
            .unwrap()
            .as_deref(),
            Some("generic")
        );
        for input in ["\n", "q\n", ""] {
            assert!(select_preset(
                &choices,
                Some(&suggestion),
                &mut input.as_bytes(),
                &mut Vec::new()
            )
            .unwrap()
            .is_none());
        }
        assert!(
            select_preset(&choices, None, &mut "d\n".as_bytes(), &mut Vec::new())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn selection_requires_number_and_explicit_confirmation() {
        let choices = vec![("generic".into(), "safe\x1b[2J".into())];
        for input in ["", "\n", "q\n", "1\n", "1\nn\n"] {
            assert!(select(
                &choices,
                None,
                Path::new("project"),
                false,
                "project",
                &mut input.as_bytes(),
                &mut Vec::new()
            )
            .unwrap()
            .is_none());
        }
        for input in ["0\ny\n", "2\ncustom-name\ny\n", "text\ny\n"] {
            assert!(select(
                &choices,
                None,
                Path::new("project"),
                false,
                "project",
                &mut input.as_bytes(),
                &mut Vec::new()
            )
            .unwrap()
            .is_none());
        }
        let mut output = Vec::new();
        assert_eq!(
            select(
                &choices,
                None,
                Path::new("project"),
                false,
                "project",
                &mut "1\n\ny\n".as_bytes(),
                &mut output
            )
            .unwrap()
            .map(|selection| selection.preset),
            Some("generic".into())
        );
        assert!(!output.contains(&27));
        assert!(answer(&mut "x".repeat(128).as_bytes()).is_err());
    }
}
