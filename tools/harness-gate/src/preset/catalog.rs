//! Generic built-in preset and explicitly selected, data-only external catalogs.
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path};

pub(super) struct Preset {
    pub(super) flow: String,
    pub(super) recipe: Option<String>,
    pub(super) packs: Vec<(String, String)>,
}

pub(super) const AUDIT_TEMPLATE: &str = include_str!("../../presets/empty.audit.toml");
pub(super) const SECRETS_TEMPLATE: &str = include_str!("../../presets/default.secrets.toml");
pub(super) const GITIGNORE_TEMPLATE: &str = "reports/\n";
const GENERIC_FLOW: &str = include_str!("../../presets/generic.flow.toml");
const GENERIC_DESCRIPTION: &str = "Git-based project with a minimal diff check";
const MAX_FILE_BYTES: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema: String,
    presets: BTreeMap<String, Entry>,
    packs: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    description: String,
    flow: String,
    recipe: Option<String>,
    #[serde(default)]
    detect_files: Vec<String>,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.starts_with(|c: char| c.is_ascii_lowercase())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn read(root: &Path, relative: &str) -> Result<String> {
    ensure!(
        !relative.is_empty()
            && !relative.contains(['\\', ':'])
            && Path::new(relative)
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "preset file must be a relative path inside the catalog: {relative}"
    );
    let mut path = root.to_path_buf();
    for part in Path::new(relative).components() {
        path.push(part);
        ensure!(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "preset file path contains a symbolic link: {relative}"
        );
    }
    ensure!(
        fs::metadata(&path)?.is_file(),
        "preset input is not a regular file: {relative}"
    );
    let mut source = String::new();
    fs::File::open(&path)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_string(&mut source)?;
    ensure!(
        source.len() as u64 <= MAX_FILE_BYTES,
        "preset input exceeds 1 MiB: {relative}"
    );
    Ok(source)
}

fn load(directory: &Path) -> Result<(std::path::PathBuf, Catalog)> {
    let root = directory
        .canonicalize()
        .context("resolve external presets directory")?;
    let source = read(&root, "catalog.json").context("read external preset catalog")?;
    let value = harness_gate::quality::parse(&source)?;
    validate_detection_metadata(&value)?;
    let catalog: Catalog =
        serde_json::from_value(value).context("decode external preset catalog")?;
    ensure!(
        matches!(
            catalog.schema.as_str(),
            "harness-preset-catalog/v1" | "harness-preset-catalog/v2"
        ),
        "unsupported preset catalog schema"
    );
    ensure!(
        catalog.presets.len() <= 64 && catalog.packs.len() <= 64,
        "preset catalog exceeds 64 entries per group"
    );
    for (name, entry) in &catalog.presets {
        ensure!(
            identifier(name) && name != "generic",
            "invalid or reserved preset name: {name}"
        );
        ensure!(
            !entry.description.trim().is_empty(),
            "preset description is empty: {name}"
        );
    }
    ensure!(
        catalog.packs.keys().all(|name| identifier(name)),
        "invalid pack name"
    );
    Ok((root, catalog))
}

pub fn print_presets(directory: Option<&Path>) -> Result<()> {
    // Validate external inputs before printing any list, so a bad path never looks successful.
    let external = directory.map(load).transpose()?;
    println!("{:<24} {}", "generic", GENERIC_DESCRIPTION);
    if let Some((_, catalog)) = external {
        for (name, entry) in catalog.presets {
            println!("{:<24} {}", name, entry.description);
        }
    }
    Ok(())
}

/// Validate every reference against the verified private file inventory.
pub(crate) fn validate(directory: &Path) -> Result<()> {
    let (root, catalog) = load(directory)?;
    ensure!(catalog.schema == "harness-preset-catalog/v1", "locked plugin catalogs require harness-preset-catalog/v1; v2 recommendation metadata is supported only by explicitly selected external directories");
    for entry in catalog.presets.values() {
        crate::config::FlowConfig::from_source(&read(&root, &entry.flow)?)?;
        if let Some(recipe) = &entry.recipe {
            read(&root, recipe)?;
        }
    }
    for pack in catalog.packs.values() {
        read(&root, pack)?;
    }
    Ok(())
}

pub(super) fn find(name: &str, directory: Option<&Path>) -> Result<Preset> {
    if name == "generic" {
        // An explicit bad catalog must not silently fall back to embedded data.
        if let Some(directory) = directory {
            load(directory)?;
        }
        return Ok(Preset {
            flow: GENERIC_FLOW.into(),
            recipe: None,
            packs: Vec::new(),
        });
    }
    let directory = directory.context("ecosystem presets require --presets-dir PATH or HARNESS_GATE_PRESETS_DIR; use a reviewed GateProbes presets directory")?;
    let (root, catalog) = load(directory)?;
    let entry = catalog.presets.get(name).with_context(|| {
        format!("unknown preset {name:?}; run `harness-gate presets --presets-dir PATH`")
    })?;
    let flow = read(&root, &entry.flow).context("read preset flow")?;
    let recipe = entry
        .recipe
        .as_ref()
        .map(|path| read(&root, path))
        .transpose()?;
    let packs = catalog
        .packs
        .iter()
        .map(|(name, path)| Ok((name.clone(), read(&root, path)?)))
        .collect::<Result<Vec<_>>>()?;
    Ok(Preset {
        flow,
        recipe,
        packs,
    })
}

pub(super) struct Recommendation {
    pub(super) name: String,
    pub(super) markers: Vec<String>,
}

pub(super) struct Menu {
    pub(super) choices: Vec<(String, String)>,
    pub(super) recommendation: Option<Recommendation>,
}

/// Only an explicitly selected external catalog may supply recommendations.
pub(super) fn choices(directory: Option<&Path>, target: Option<&Path>) -> Result<Menu> {
    let external = directory.map(load).transpose()?;
    let mut menu = Menu {
        choices: vec![("generic".into(), GENERIC_DESCRIPTION.into())],
        recommendation: None,
    };
    if let Some((_, catalog)) = external {
        menu.recommendation = target.and_then(|target| recommend(&catalog, target));
        menu.choices.extend(
            catalog
                .presets
                .into_iter()
                .map(|(name, entry)| (name, entry.description)),
        );
    }
    Ok(menu)
}

fn recommend(catalog: &Catalog, target: &Path) -> Option<Recommendation> {
    let mut matches = catalog.presets.iter().filter(|(_, entry)| {
        !entry.detect_files.is_empty()
            && entry.detect_files.iter().all(|name| {
                fs::symlink_metadata(target.join(name))
                    .is_ok_and(|metadata| metadata.file_type().is_file())
            })
    });
    let (name, entry) = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(Recommendation {
        name: name.clone(),
        markers: entry.detect_files.clone(),
    })
}

fn validate_detection_metadata(value: &serde_json::Value) -> Result<()> {
    let Some(entries) = value["presets"].as_object() else {
        return Ok(());
    };
    for entry in entries.values() {
        let Some(markers) = entry.get("detect_files") else {
            continue;
        };
        ensure!(
            value["schema"] == "harness-preset-catalog/v2",
            "detect_files requires harness-preset-catalog/v2"
        );
        let markers = markers
            .as_array()
            .context("detect_files must be a nonempty list of known root marker files")?;
        ensure!(
            !markers.is_empty() && markers.len() <= 4,
            "detect_files must contain 1 to 4 unique known root markers"
        );
        let mut unique = std::collections::BTreeSet::new();
        for marker in markers {
            let name = marker
                .as_str()
                .context("detect_files entries must be strings")?;
            ensure!(
                matches!(
                    name,
                    "Cargo.toml" | "package.json" | "pyproject.toml" | "tsconfig.json"
                ),
                "unsupported project marker: {name:?}"
            );
            ensure!(unique.insert(name), "duplicate project marker: {name}");
        }
    }
    Ok(())
}
