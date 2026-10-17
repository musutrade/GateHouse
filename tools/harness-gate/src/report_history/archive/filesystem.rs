use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 8192;

pub(super) fn relative(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && !path.contains(['\\', ':', '\0'])
        && path.split('/').all(|part| {
            !part.is_empty()
                && part.trim() == part
                && !part.ends_with('.')
                && !part.eq_ignore_ascii_case(".git")
        })
}
pub(super) fn directory(root: &Path, relative_path: &str) -> Result<PathBuf> {
    ensure!(relative(relative_path), "unsafe archive directory");
    let mut path = root.canonicalize()?;
    for part in relative_path.split('/') {
        path.push(part);
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        ensure!(
            fs::symlink_metadata(&path)?.is_dir(),
            "archive directory cannot be a link or non-directory"
        );
    }
    Ok(path)
}
pub(super) struct Lock {
    path: PathBuf,
    token: Vec<u8>,
    _file: fs::File,
}
impl Lock {
    pub(super) fn acquire(root: &Path) -> Result<Self> {
        let path = root.join(".archive.lock");
        let mut temp = tempfile::NamedTempFile::new_in(root)?;
        let token = temp
            .path()
            .file_name()
            .context("lock name")?
            .to_string_lossy()
            .as_bytes()
            .to_vec();
        temp.write_all(&token)?;
        temp.as_file().sync_all()?;
        let file = temp.persist_noclobber(&path).map_err(|e| e.error).context(
            "history archive busy or stale lock; inspect before removing the lock manually",
        )?;
        Ok(Self {
            path,
            token,
            _file: file,
        })
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path)
            .is_ok_and(|m| m.is_file() && m.len() == self.token.len() as u64)
            && fs::read(&self.path).is_ok_and(|bytes| bytes == self.token)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}
pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().context("archive parent")?)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(path).map_err(|e| e.error)?;
    Ok(())
}
pub(super) fn replace(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        ensure!(meta.is_file(), "history index must be a regular file");
    }
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().context("index parent")?)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    // tempfile uses atomic replacement on Windows too, never remove-then-rename.
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

pub(super) fn inventory(root: &Path) -> Result<(Vec<PathBuf>, u64)> {
    let mut directories = vec![root.to_path_buf()];
    let mut files = Vec::new();
    let mut entries = 0;
    let mut total = 0;
    while let Some(directory) = directories.pop() {
        ensure!(
            fs::symlink_metadata(&directory)?.is_dir(),
            "archive tree contains a non-directory"
        );
        for entry in fs::read_dir(&directory)? {
            entries += 1;
            ensure!(
                entries <= MAX_ENTRIES,
                "archive snapshot exceeds 8192 filesystem entries"
            );
            let entry = entry?;
            let path = entry.path();
            let relative_path = path.strip_prefix(root)?;
            ensure!(
                relative_path.components().count() <= 32,
                "archive path nesting exceeds 32"
            );
            let meta = fs::symlink_metadata(&path)?;
            ensure!(
                meta.is_dir() || meta.is_file(),
                "archive rejects links and special files"
            );
            if meta.is_dir() {
                directories.push(path);
            } else {
                total += meta.len();
                ensure!(total <= MAX_BYTES, "archive snapshot exceeds 256 MiB");
                files.push(relative_path.to_path_buf());
            }
        }
    }
    files.sort();
    Ok((files, total))
}
pub(super) fn copy(source: &Path, target: &Path, files: &[PathBuf]) -> Result<()> {
    let mut total = 0;
    for relative_path in files {
        let from = source.join(relative_path);
        let to = target.join(relative_path);
        if let Some(parent) = relative_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            directory(target, &parent.to_string_lossy().replace('\\', "/"))?;
        }
        ensure!(
            fs::symlink_metadata(&from)?.is_file(),
            "archive input changed to a non-file"
        );
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&to)?;
        total += std::io::copy(
            &mut fs::File::open(from)?.take(MAX_BYTES - total + 1),
            &mut output,
        )?;
        ensure!(total <= MAX_BYTES, "archive input grew beyond 256 MiB");
        output.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locks_preserve_conflicts_and_owned_files_publish_without_clobbering() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let lock = Lock::acquire(root).unwrap();
        assert!(Lock::acquire(root).is_err());
        fs::write(root.join(".archive.lock"), "foreign").unwrap();
        drop(lock);
        assert_eq!(fs::read(root.join(".archive.lock")).unwrap(), b"foreign");
        fs::remove_file(root.join(".archive.lock")).unwrap();
        drop(Lock::acquire(root).unwrap());
        assert!(!root.join(".archive.lock").exists());
        let file = root.join("index.json");
        write_new(&file, b"first").unwrap();
        assert!(write_new(&file, b"second").is_err());
        assert_eq!(fs::read(&file).unwrap(), b"first");
        replace(&file, b"second").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"second");
        assert!(replace(root, b"not a file").is_err());
    }
    #[test]
    fn copied_files_are_independent_and_tree_limits_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        let source = directory(temp.path(), "source").unwrap();
        let target = directory(temp.path(), "target").unwrap();
        let nested = directory(&source, "nested").unwrap();
        fs::write(nested.join("a"), b"original").unwrap();
        let (files, total) = inventory(&source).unwrap();
        assert_eq!(total, 8);
        copy(&source, &target, &files).unwrap();
        fs::write(nested.join("a"), b"changed").unwrap();
        assert_eq!(fs::read(target.join("nested/a")).unwrap(), b"original");
        assert!(copy(&source, &target, &files).is_err());
        let large = fs::File::create(source.join("large")).unwrap();
        large.set_len(MAX_BYTES + 1).unwrap();
        assert!(inventory(&source).is_err());
        for path in ["", "..", "a/../b", "/a", "a//b", ".git/x", "C:/a", "a\\b"] {
            assert!(directory(temp.path(), path).is_err());
        }
        assert!(directory(&source, "nested/a/sub").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_inputs_destinations_and_indexes_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let source = directory(root, "source").unwrap();
        std::os::unix::fs::symlink(&source, root.join("alias")).unwrap();
        assert!(directory(root, "alias/nested").is_err());
        fs::write(root.join("original"), b"keep").unwrap();
        std::os::unix::fs::symlink(root.join("original"), source.join("link")).unwrap();
        assert!(inventory(&source).is_err());
        assert!(replace(&source.join("link"), b"bad").is_err());
        assert_eq!(fs::read(root.join("original")).unwrap(), b"keep");
    }
}
