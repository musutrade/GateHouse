//! Portable regular-file input manifests. Empty directories and metadata other
//! than the executable bit are deliberately outside the content identity.
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_FILES: usize = 100_000;
const MAX_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ContentManifest {
    pub version: u32,
    pub files: Vec<ContentFile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ContentFile {
    pub path: String,
    pub kind: &'static str,
    pub mode: u32,
    pub bytes: u64,
    pub sha256: String,
}

impl ContentManifest {
    /// The caller chooses an explicit output subtree to omit. No ignore file,
    /// SCM command, ambient environment or network influences the file set.
    pub(crate) fn capture(root: &Path, output: Option<&Path>) -> Result<Self> {
        ensure!(root.is_dir(), "candidate root is not a directory");
        let mut files = Vec::new();
        let mut directories = vec![PathBuf::new()];
        let mut total = 0u64;
        let mut entries = 0usize;
        while let Some(relative) = directories.pop() {
            for entry in fs::read_dir(root.join(&relative))? {
                let entry = entry?;
                entries += 1;
                ensure!(entries <= MAX_FILES, "candidate exceeds entry limit");
                let path = relative.join(entry.file_name());
                if output.is_some_and(|output| path == output) {
                    continue;
                }
                // Only the top-level Git administrative entry is metadata.
                // Nested repositories/submodules are not regular input trees.
                if entry.file_name() == ".git" {
                    ensure!(
                        relative.as_os_str().is_empty(),
                        "nested Git repository is unsupported"
                    );
                    continue;
                }
                let name = portable_path(&path)?;
                let metadata = fs::symlink_metadata(entry.path())?;
                ensure!(
                    !metadata.file_type().is_symlink(),
                    "candidate contains symlink: {name}"
                );
                if metadata.is_dir() {
                    directories.push(path);
                    continue;
                }
                ensure!(
                    metadata.is_file(),
                    "candidate contains non-regular file: {name}"
                );
                ensure!(files.len() < MAX_FILES, "candidate exceeds file limit");
                total = total
                    .checked_add(metadata.len())
                    .context("candidate size overflow")?;
                ensure!(total <= MAX_BYTES, "candidate exceeds 1 GiB input limit");
                let mut file = fs::File::open(entry.path())?;
                let mut hasher = Sha256::new();
                let copied = std::io::copy(&mut (&mut file).take(metadata.len() + 1), &mut hasher)?;
                ensure!(
                    copied == metadata.len(),
                    "candidate changed while hashing: {name}"
                );
                files.push(ContentFile {
                    path: name,
                    kind: "file",
                    mode: normalized_mode(&metadata),
                    bytes: copied,
                    sha256: format!("sha256:{:x}", hasher.finalize()),
                });
            }
        }
        files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
        Ok(Self { version: 1, files })
    }

    pub(crate) fn identity(&self) -> Result<String> {
        let bytes = serde_json::to_vec(self)?;
        Ok(format!("content-v1:sha256:{:x}", Sha256::digest(bytes)))
    }

    pub(crate) fn materialize(&self, source: &Path, destination: &Path) -> Result<()> {
        for entry in &self.files {
            let path = destination.join(&entry.path);
            fs::create_dir_all(path.parent().context("candidate file has no parent")?)?;
            let from = source.join(&entry.path);
            ensure!(
                fs::symlink_metadata(&from)?.is_file(),
                "candidate file type changed"
            );
            let mut input = fs::File::open(&from)?;
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            let copied = std::io::copy(&mut (&mut input).take(entry.bytes + 1), &mut output)?;
            ensure!(copied == entry.bytes, "candidate changed while copying");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(entry.mode))?;
            }
        }
        ensure!(
            &Self::capture(destination, None)? == self,
            "materialized candidate content changed"
        );
        Ok(())
    }
}

fn portable_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(value) = component else {
            anyhow::bail!("candidate path must be relative");
        };
        let value = value.to_str().context("candidate path is not UTF-8")?;
        ensure!(
            !value.contains(['\\', ':']),
            "candidate path is not portable"
        );
        parts.push(value);
    }
    ensure!(!parts.is_empty(), "empty candidate path");
    Ok(parts.join("/"))
}

fn normalized_mode(metadata: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 != 0 {
            return 0o755;
        }
    }
    let _ = metadata;
    0o644
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_depends_on_content_not_location_or_git_metadata() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        for root in [a.path(), b.path()] {
            fs::create_dir(root.join("src")).unwrap();
            fs::write(root.join("src/input"), b"candidate").unwrap();
        }
        fs::create_dir(a.path().join("empty")).unwrap();
        fs::write(a.path().join(".git"), b"gitdir: arbitrary-location").unwrap();
        let expected = ContentManifest::capture(a.path(), None).unwrap();
        assert_eq!(expected, ContentManifest::capture(b.path(), None).unwrap());
        fs::write(b.path().join("src/input"), b"different").unwrap();
        assert_ne!(
            expected.identity().unwrap(),
            ContentManifest::capture(b.path(), None)
                .unwrap()
                .identity()
                .unwrap()
        );
    }

    #[test]
    fn materialization_checks_bytes_and_omits_only_explicit_output() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        fs::write(source.path().join("input"), b"fixed").unwrap();
        fs::create_dir(source.path().join("reports")).unwrap();
        fs::write(source.path().join("reports/old"), b"output").unwrap();
        let manifest = ContentManifest::capture(source.path(), Some(Path::new("reports"))).unwrap();
        manifest
            .materialize(source.path(), destination.path())
            .unwrap();
        assert!(!destination.path().join("reports").exists());
        assert_eq!(
            manifest,
            ContentManifest::capture(destination.path(), None).unwrap()
        );
        fs::write(source.path().join("input"), b"other").unwrap();
        let changed = tempfile::tempdir().unwrap();
        assert!(manifest.materialize(source.path(), changed.path()).is_err());
        assert!(manifest
            .materialize(source.path(), destination.path())
            .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn permissions_normalize_and_links_are_rejected() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("input");
        fs::write(&path, "bytes").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let plain = ContentManifest::capture(root.path(), None).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(plain, ContentManifest::capture(root.path(), None).unwrap());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        assert_ne!(plain, ContentManifest::capture(root.path(), None).unwrap());
        symlink("input", root.path().join("link")).unwrap();
        assert!(ContentManifest::capture(root.path(), None).is_err());
    }

    #[test]
    fn nested_repository_is_not_silently_omitted() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(root.path().join("nested/.git"), "metadata").unwrap();
        assert!(ContentManifest::capture(root.path(), None).is_err());
    }
}
