use super::filesystem::{atomic_write_batch, resolve_inside};
use crate::config::import::{import_execution, RUNTIME_BLOCKERS};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Component, Path};

pub(crate) fn import_arc_flow(
    root: &Path,
    input: &Path,
    output: &Path,
    execution_only: bool,
) -> Result<()> {
    if !execution_only {
        bail!("full Arc-Flow migration is blocked:\n{}\nUse --execution-only to import declared configuration for review; this does not authorize replacing existing gates.", RUNTIME_BLOCKERS.join("\n"));
    }
    for path in [input, output] {
        if path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            bail!(
                "import paths must not contain parent traversal: {}",
                path.display()
            );
        }
    }
    let root = root.canonicalize().context("resolve import project root")?;
    let input = resolve_inside(&root, input.to_owned())?;
    let output = resolve_inside(&root, output.to_owned())?;
    let report_path = output.with_extension("import.json");
    // Import never overwrites source, existing configuration or an earlier report.
    for path in [&output, &report_path] {
        if fs::symlink_metadata(path).is_ok() {
            bail!(
                "{} already exists; choose an unused output path",
                path.display()
            );
        }
    }
    let source = fs::read_to_string(&input).context("read Arc-Flow execution configuration")?;
    let imported = import_execution(&source)?;
    let report = serde_json::to_vec_pretty(&imported.report)?;
    atomic_write_batch(&[
        (&output, imported.source.as_bytes()),
        (&report_path, &report),
    ])?;
    println!("Imported execution configuration: {}", output.display());
    println!("Parity inventory and UX metrics: {}", report_path.display());
    println!(
        "Authority transfer BLOCKED; runtime incompatibilities:\n{}",
        RUNTIME_BLOCKERS.join("\n")
    );
    Ok(())
}
