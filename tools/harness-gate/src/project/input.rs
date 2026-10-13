use crate::utils::git;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

static SNAPSHOT_COUNTER: AtomicU64 = AtomicU64::new(1);

/// The source interpretation attached to one verification invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum InputMode {
    WorkingTree,
    Staged,
    Base,
    All,
}

impl InputMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::WorkingTree => "working-tree",
            Self::Staged => "staged",
            Self::Base => "base",
            Self::All => "all",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct InvocationInput {
    pub(crate) mode: InputMode,
    pub(crate) project_identity: String,
    pub(crate) source_identity: String,
    pub(crate) execution_root: PathBuf,
    pub(crate) configuration_digest: String,
    #[serde(skip)]
    snapshot: Option<Arc<SnapshotGuard>>,
}

#[derive(Debug)]
struct SnapshotGuard {
    root: PathBuf,
}

impl Drop for SnapshotGuard {
    fn drop(&mut self) {
        // This path is created exclusively by `materialize_staged`; never
        // follow a link while removing a failed or completed snapshot.
        if fs::symlink_metadata(&self.root)
            .map(|metadata| metadata.file_type().is_dir() && !metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

impl InvocationInput {
    pub(crate) fn working_tree(repository_root: &Path, config_bytes: &[u8]) -> Result<Self> {
        let root = repository_root
            .canonicalize()
            .with_context(|| format!("resolve project root {}", repository_root.display()))?;
        let head = git::capture(&root, ["rev-parse", "HEAD"])
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".into());
        Ok(Self {
            mode: InputMode::WorkingTree,
            project_identity: root.to_string_lossy().into_owned(),
            source_identity: format!("working-tree:{head}"),
            execution_root: root,
            configuration_digest: digest_bytes(config_bytes),
            snapshot: None,
        })
    }

    pub(crate) fn materialize_staged(
        repository_root: &Path,
        configuration_path: &Path,
    ) -> Result<Self> {
        let root = repository_root
            .canonicalize()
            .with_context(|| format!("resolve project root {}", repository_root.display()))?;
        let tree = git::capture(&root, ["write-tree"]).context("materialize staged Git index")?;
        if !tree.status.success() {
            bail!(
                "cannot materialize staged Git index: {}",
                String::from_utf8_lossy(&tree.stderr).trim()
            );
        }
        let tree_id = String::from_utf8(tree.stdout)
            .context("Git returned a non-UTF-8 staged tree identity")?
            .trim()
            .to_string();
        if tree_id.is_empty() {
            bail!("Git returned an empty staged tree identity");
        }

        let snapshot_root = allocate_snapshot_root()?;
        let prefix = format!("{}/", snapshot_root.to_string_lossy());
        let prefix_argument = format!("--prefix={prefix}");
        let checkout = git::capture(&root, ["checkout-index", "--all", prefix_argument.as_str()])?;
        if !checkout.status.success() {
            let _ = fs::remove_dir_all(&snapshot_root);
            bail!(
                "cannot materialize staged Git index: {}",
                String::from_utf8_lossy(&checkout.stderr).trim()
            );
        }
        let guard = Arc::new(SnapshotGuard {
            root: snapshot_root.clone(),
        });
        reject_snapshot_links(&snapshot_root)?;
        let config_bytes = fs::read(snapshot_root.join(configuration_path)).with_context(|| {
            format!(
                "read staged workflow configuration {}",
                configuration_path.display()
            )
        })?;
        let config_digest = digest_bytes(&config_bytes);
        Ok(Self {
            mode: InputMode::Staged,
            project_identity: root.to_string_lossy().into_owned(),
            source_identity: format!("git-tree:{tree_id}"),
            execution_root: snapshot_root,
            configuration_digest: config_digest,
            snapshot: Some(guard),
        })
    }

    pub(crate) fn is_snapshot(&self) -> bool {
        self.snapshot.is_some()
    }
}

fn allocate_snapshot_root() -> Result<PathBuf> {
    // Configuration containment checks compare canonical paths. Resolve the
    // temporary parent too (for example, /var -> /private/var on macOS).
    let base = std::env::temp_dir()
        .canonicalize()
        .context("resolve staged snapshot temporary directory")?;
    for _ in 0..64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let id = SNAPSHOT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = base.join(format!(
            "harness-gate-staged-{}-{}-{}",
            std::process::id(),
            now.as_nanos(),
            id
        ));
        #[cfg(unix)]
        let created = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700).create(&path)
        };
        #[cfg(not(unix))]
        let created = fs::create_dir(&path);
        match created {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("create staged snapshot {}", path.display()));
            }
        }
    }
    bail!("could not allocate a collision-free staged snapshot directory")
}

fn reject_snapshot_links(root: &Path) -> Result<()> {
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)
            .with_context(|| format!("read staged snapshot {}", directory.display()))?
        {
            let entry = entry.context("read staged snapshot entry")?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .with_context(|| format!("inspect staged snapshot entry {}", path.display()))?;
            if metadata.file_type().is_symlink() {
                let _ = fs::remove_dir_all(root);
                bail!(
                    "staged snapshot contains a symbolic link: {}",
                    path.display()
                );
            }
            if metadata.is_dir() {
                directories.push(path);
            } else if !metadata.is_file() {
                let _ = fs::remove_dir_all(root);
                bail!(
                    "staged snapshot contains a non-regular entry: {}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("sha256:{:x}", digest.finalize())
}
