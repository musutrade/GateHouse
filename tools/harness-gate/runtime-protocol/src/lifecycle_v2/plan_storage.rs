//! Create-new evidence storage. Errors retain files; never retry or repair them.
use super::{plan_record::PlanRecord, protected_context::ProtectedContext};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, IntoRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, &'static str>;
const LIMIT: u64 = 16 * 1024 * 1024;
fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|_| "plan evidence filesystem failure")
}
fn identity(m: &fs::Metadata) -> (u64, u64, u32, u32, u32, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.mode(),
        m.uid(),
        m.gid(),
        m.nlink(),
        m.ctime(),
        m.ctime_nsec(),
        m.mtime(),
        m.mtime_nsec(),
    )
}
struct Parent {
    path: PathBuf,
    file: File,
    dev: u64,
    ino: u64,
}
impl Parent {
    fn open(path: &Path) -> Result<(Self, PathBuf)> {
        let parent = path.parent().ok_or("record parent required")?;
        let name = path.file_name().ok_or("record name required")?;
        if !path.is_absolute()
            || path.to_str().is_none_or(|s| {
                s.len() > 4096
                    || s.chars().any(char::is_control)
                    || s.split('/')
                        .skip(1)
                        .any(|p| p.is_empty() || p == "." || p == "..")
            })
        {
            return Err("canonical record path required");
        }
        check_parent(parent)?;
        let file = io(OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(parent))?;
        let m = io(file.metadata())?;
        let held = Self {
            path: parent.into(),
            dev: m.dev(),
            ino: m.ino(),
            file,
        };
        held.check()?;
        let anchored = PathBuf::from(format!("/proc/self/fd/{}", held.file.as_raw_fd())).join(name);
        Ok((held, anchored))
    }
    fn check(&self) -> Result<()> {
        check_parent(&self.path)?;
        let m = io(fs::symlink_metadata(&self.path))?;
        let held = io(self.file.metadata())?;
        if (m.dev(), m.ino()) != (self.dev, self.ino) || identity(&m) != identity(&held) {
            return Err("record parent replaced");
        }
        Ok(())
    }
}
fn check_parent(path: &Path) -> Result<()> {
    if io(fs::canonicalize(path))? != path {
        return Err("record parent symlink");
    }
    for p in path.ancestors() {
        let m = io(fs::symlink_metadata(p))?;
        if !m.is_dir()
            || m.mode() & 0o022 != 0
            || ![0, unsafe { libc::geteuid() }].contains(&m.uid())
        {
            return Err("unprotected record parent");
        }
    }
    Ok(())
}
fn valid(m: &fs::Metadata) -> Result<()> {
    if !m.is_file()
        || m.mode() & 0o7777 != 0o600
        || m.uid() != unsafe { libc::geteuid() }
        || m.nlink() != 1
        || m.len() > LIMIT
    {
        return Err("unprotected record file");
    }
    Ok(())
}
fn close(file: File) -> Result<()> {
    // Do not retry close after EINTR: Linux already releases the descriptor.
    if unsafe { libc::close(file.into_raw_fd()) } != 0 {
        return Err("record close failed");
    }
    Ok(())
}
impl PlanRecord {
    /// Not a journal or lease. A failed call can leave a complete or partial file.
    pub fn create_new(&self, path: &Path, context: &ProtectedContext) -> Result<()> {
        self.create_checked(path, context, || Ok(()))
    }
    pub(super) fn create_checked(
        &self,
        path: &Path,
        context: &ProtectedContext,
        after_write: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let bytes = self.encode()?;
        Self::decode(&bytes, context)?;
        write_new(path, &bytes, |_| context.revalidate())?;
        after_write()?;
        if Self::open(path, context)?.encode()? != bytes {
            return Err("created record differs from intended evidence");
        }
        Ok(())
    }
    pub fn open(path: &Path, context: &ProtectedContext) -> Result<Self> {
        let (parent, anchored) = Parent::open(path)?;
        let file = io(OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(&anchored))?;
        let before = io(file.metadata())?;
        valid(&before)?;
        let mut bytes = Vec::new();
        io((&file).take(LIMIT + 1).read_to_end(&mut bytes))?;
        if bytes.len() as u64 > LIMIT {
            return Err("record too large");
        }
        let record = Self::decode(&bytes, context)?;
        parent.check()?;
        if identity(&before) != identity(&io(file.metadata())?)
            || identity(&before) != identity(&io(fs::symlink_metadata(&anchored))?)
        {
            return Err("record changed during read");
        }
        close(file)?;
        context.revalidate()?;
        Ok(record)
    }
}
trait StorageIo {
    fn write(&mut self, file: &mut File, bytes: &[u8]) -> std::io::Result<()> {
        file.write_all(bytes)
    }
    fn sync(&mut self, file: &File, _parent: bool) -> std::io::Result<()> {
        file.sync_all()
    }
    fn close(&mut self, file: File) -> Result<()> {
        close(file)
    }
}
struct SystemIo;
impl StorageIo for SystemIo {}
fn write_new(path: &Path, bytes: &[u8], boundary: impl FnMut(&str) -> Result<()>) -> Result<()> {
    write_with_io(path, bytes, boundary, &mut SystemIo)
}
fn write_with_io(
    path: &Path,
    bytes: &[u8],
    mut boundary: impl FnMut(&str) -> Result<()>,
    ops: &mut impl StorageIo,
) -> Result<()> {
    let (parent, anchored) = Parent::open(path)?;
    boundary("before-create")?;
    let mut file = io(OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&anchored))?;
    let initial = io(file.metadata())?;
    valid(&initial)?;
    boundary("created")?;
    let middle = bytes.len() / 2;
    io(ops.write(&mut file, &bytes[..middle]))?;
    boundary("partial-write")?;
    io(ops.write(&mut file, &bytes[middle..]))?;
    boundary("before-file-sync")?;
    io(ops.sync(&file, false))?;
    boundary("before-parent-sync")?;
    parent.check()?;
    let final_meta = io(file.metadata())?;
    valid(&final_meta)?;
    if (initial.dev(), initial.ino()) != (final_meta.dev(), final_meta.ino())
        || identity(&final_meta) != identity(&io(fs::symlink_metadata(&anchored))?)
        || final_meta.len() != bytes.len() as u64
    {
        return Err("record replaced during write");
    }
    io(ops.sync(&parent.file, true))?;
    boundary("before-close")?;
    ops.close(file)?;
    parent.check()?;
    boundary("complete")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> tempfile::TempDir {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::Builder::new()
            .prefix("p103-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }
    #[test]
    fn interruption_retains_evidence_and_never_overwrites() {
        for stage in [
            "before-create",
            "created",
            "partial-write",
            "before-file-sync",
            "before-parent-sync",
            "before-close",
            "complete",
        ] {
            let root = root();
            let path = root.path().join("record");
            assert!(write_new(&path, b"evidence", |s| if s == stage {
                Err("injected interruption")
            } else {
                Ok(())
            })
            .is_err());
            if stage == "before-create" {
                assert!(!path.exists());
            } else {
                let prior = fs::read(&path).unwrap();
                assert!(write_new(&path, b"replacement", |_| Ok(())).is_err());
                assert_eq!(fs::read(&path).unwrap(), prior);
            }
        }
    }
    #[test]
    fn concurrent_create_has_one_winner_and_symlink_refuses() {
        let root = root();
        let path = root.path().join("record");
        std::thread::scope(|s| {
            let a = s.spawn(|| write_new(&path, b"one", |_| Ok(())));
            let b = s.spawn(|| write_new(&path, b"two", |_| Ok(())));
            assert_ne!(a.join().unwrap().is_ok(), b.join().unwrap().is_ok());
        });
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(write_new(&link, b"bad", |_| Ok(())).is_err());
    }
    struct Fault(&'static str);
    impl StorageIo for Fault {
        fn write(&mut self, file: &mut File, bytes: &[u8]) -> std::io::Result<()> {
            if self.0 == "short-write" {
                file.write_all(&bytes[..1])?;
                return Err(std::io::Error::from_raw_os_error(libc::ENOSPC));
            }
            file.write_all(bytes)
        }
        fn sync(&mut self, file: &File, parent: bool) -> std::io::Result<()> {
            if self.0 == if parent { "parent-sync" } else { "file-sync" } {
                return Err(std::io::Error::from_raw_os_error(libc::EIO));
            }
            file.sync_all()
        }
        fn close(&mut self, file: File) -> Result<()> {
            close(file)?;
            if self.0 == "close" {
                Err("injected close error after descriptor release")
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn io_failures_return_failure_and_retain_without_retry() {
        for stage in ["short-write", "file-sync", "parent-sync", "close"] {
            let root = root();
            let path = root.path().join("record");
            assert!(write_with_io(&path, b"evidence", |_| Ok(()), &mut Fault(stage)).is_err());
            let retained = fs::read(&path).unwrap();
            assert_eq!(
                retained,
                if stage == "short-write" {
                    b"e".as_slice()
                } else {
                    b"evidence".as_slice()
                }
            );
            assert!(write_new(&path, b"retry", |_| Ok(())).is_err());
            assert_eq!(fs::read(&path).unwrap(), retained);
        }
    }
    #[test]
    fn replacement_during_write_is_rejected() {
        let root = root();
        let path = root.path().join("record");
        assert!(write_new(&path, b"evidence", |stage| {
            if stage == "before-parent-sync" {
                fs::rename(&path, root.path().join("old")).unwrap();
                fs::write(&path, b"replacement").unwrap();
            }
            Ok(())
        })
        .is_err());
    }
    #[test]
    fn parent_replacement_during_write_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let root = root();
        let parent = root.path().join("parent");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        let path = parent.join("record");
        assert!(write_new(&path, b"evidence", |stage| {
            if stage == "before-parent-sync" {
                fs::rename(&parent, root.path().join("old-parent")).unwrap();
                fs::create_dir(&parent).unwrap();
                fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
            }
            Ok(())
        })
        .is_err());
        assert!(!path.exists());
        assert_eq!(
            fs::read(root.path().join("old-parent/record")).unwrap(),
            b"evidence"
        );
    }
}
