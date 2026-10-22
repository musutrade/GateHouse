//! Linux filesystem checkpoint only: protected config bytes and held socket inode.
//! An O_PATH descriptor is not a connected transport or peer authentication.
use super::{
    native_json,
    observation::{ExpectedContext, SocketObservation},
};
use sha2::{Digest, Sha256};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, &'static str>;
fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|_| "protected Podman filesystem rejected")
}
#[derive(PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    len: u64,
    ctime: i64,
    ctime_ns: i64,
    mtime: i64,
    mtime_ns: i64,
}
impl Identity {
    fn of(m: &fs::Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            mode: m.mode(),
            uid: m.uid(),
            gid: m.gid(),
            links: m.nlink(),
            len: m.len(),
            ctime: m.ctime(),
            ctime_ns: m.ctime_nsec(),
            mtime: m.mtime(),
            mtime_ns: m.mtime_nsec(),
        }
    }
}
fn protected(path: &Path) -> Result<()> {
    let text = path.to_str().ok_or("non-UTF8 path")?;
    if !path.is_absolute()
        || text.len() > 4096
        || text.chars().any(char::is_control)
        || text
            .split('/')
            .skip(1)
            .any(|p| p.is_empty() || p == "." || p == "..")
        || io(fs::canonicalize(path))? != path
    {
        return Err("noncanonical protected path");
    }
    for parent in path.ancestors().skip(1) {
        let m = io(fs::symlink_metadata(parent))?;
        if !m.is_dir()
            || m.mode() & 0o022 != 0
            || ![0, unsafe { libc::geteuid() }].contains(&m.uid())
        {
            return Err("unprotected ancestor");
        }
    }
    Ok(())
}
struct Held {
    path: PathBuf,
    file: File,
    identity: Identity,
}
impl Held {
    fn open(path: &Path, socket: bool) -> Result<Self> {
        protected(path)?;
        let flags = libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if socket { libc::O_PATH } else { 0 };
        let file = io(OpenOptions::new().read(true).custom_flags(flags).open(path))?;
        let m = io(file.metadata())?;
        if socket {
            if m.mode() & 0o170000 != 0o140000
                || m.mode() & 0o7007 != 0
                || ![0, unsafe { libc::geteuid() }].contains(&m.uid())
            {
                return Err("unprotected socket");
            }
        } else if !m.is_file()
            || m.nlink() != 1
            || m.uid() != unsafe { libc::geteuid() }
            || !matches!(m.mode() & 0o7777, 0o400 | 0o600)
            || m.len() > native_json::LIMIT as u64
        {
            return Err("unprotected config");
        }
        let held = Self {
            path: path.into(),
            file,
            identity: Identity::of(&m),
        };
        held.check()?;
        Ok(held)
    }
    fn check(&self) -> Result<()> {
        protected(&self.path)?;
        if Identity::of(&io(self.file.metadata())?) != self.identity
            || Identity::of(&io(fs::symlink_metadata(&self.path))?) != self.identity
        {
            return Err("protected input replaced or changed");
        }
        Ok(())
    }
}
/// Exact expected context bytes supplied by the operator. No installation or
/// dispatch authority is minted here. No environment or old lease migration.
pub struct ProtectedContext {
    config: Held,
    socket: Held,
    bytes: Vec<u8>,
    context: ExpectedContext,
}
impl ProtectedContext {
    pub fn load(path: &Path) -> Result<Self> {
        let config = Held::open(path, false)?;
        let mut bytes = Vec::new();
        io((&config.file)
            .take(native_json::LIMIT as u64 + 1)
            .read_to_end(&mut bytes))?;
        let context: ExpectedContext = serde_json::from_value(native_json::decode(&bytes)?)
            .map_err(|_| "protected context schema rejected")?;
        context.validate()?;
        if context.principal_uid != unsafe { libc::geteuid() } {
            return Err("configured principal differs from euid");
        }
        let socket = Held::open(Path::new(&context.observation.socket.path), true)?;
        let held = Self {
            config,
            socket,
            bytes,
            context,
        };
        held.revalidate()?;
        Ok(held)
    }
    pub fn revalidate(&self) -> Result<()> {
        if self.context.principal_uid != unsafe { libc::geteuid() } {
            return Err("principal changed");
        }
        self.config.check()?;
        self.socket.check()?;
        let m = io(self.socket.file.metadata())?;
        let observed = SocketObservation {
            path: self
                .socket
                .path
                .to_str()
                .ok_or("socket path rejected")?
                .into(),
            device: m.dev(),
            inode: m.ino(),
            uid: m.uid(),
            mode: m.mode(),
        };
        if observed != self.context.observation.socket {
            return Err("configured socket differs from held inode");
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Result<(&ExpectedContext, &[u8])> {
        self.revalidate()?;
        Ok((&self.context, &self.bytes))
    }
    pub fn config_sha256(&self) -> Result<String> {
        self.revalidate()?;
        Ok(format!("{:x}", Sha256::digest(&self.bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::{
        fs::{symlink, PermissionsExt},
        net::UnixListener,
    };
    struct Fixture {
        dir: tempfile::TempDir,
        _listener: UnixListener,
        config: PathBuf,
        socket: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            // HOME is only a test scratch location, never production identity.
            // A protected short parent is required; assertions do not bypass checks.
            let dir = tempfile::tempdir_in(
                std::env::var_os("HOME").expect("protected test HOME required"),
            )
            .unwrap();
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
            let socket = dir.path().join("engine.sock");
            let listener = UnixListener::bind(&socket).unwrap();
            fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
            let m = fs::metadata(&socket).unwrap();
            let value = serde_json::json!({"operator_epoch":"a".repeat(32),"principal_uid":unsafe{libc::geteuid()},"observation":{"engine":"podman","api_surface":"libpod","selected_api":"6.1.4","server_version":"6.1.4","hostname":"fixture","rootless":false,"graph_root":"/var/lib/containers/storage","run_root":"/run/containers/storage","remote_socket":null,"socket":{"path":socket,"device":m.dev(),"inode":m.ino(),"uid":m.uid(),"mode":m.mode()}}});
            let config = dir.path().join("context.json");
            fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
            fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
            Self {
                dir,
                _listener: listener,
                config,
                socket,
            }
        }
    }
    #[test]
    fn exact_bytes_and_socket_snapshot() {
        let f = Fixture::new();
        let held = ProtectedContext::load(&f.config).unwrap();
        assert_eq!(held.snapshot().unwrap().1, fs::read(&f.config).unwrap());
        assert_eq!(
            held.config_sha256().unwrap(),
            format!("{:x}", Sha256::digest(fs::read(&f.config).unwrap()))
        );
    }
    #[test]
    fn config_replacement_same_bytes_is_rejected() {
        let f = Fixture::new();
        let held = ProtectedContext::load(&f.config).unwrap();
        let replacement = f.dir.path().join("new");
        fs::write(&replacement, fs::read(&f.config).unwrap()).unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
        fs::rename(replacement, &f.config).unwrap();
        assert!(held.snapshot().is_err());
    }
    #[test]
    fn socket_replacement_is_rejected() {
        let f = Fixture::new();
        let held = ProtectedContext::load(&f.config).unwrap();
        fs::remove_file(&f.socket).unwrap();
        let _new = UnixListener::bind(&f.socket).unwrap();
        fs::set_permissions(&f.socket, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(held.revalidate().is_err());
    }
    #[test]
    fn config_socket_and_ancestor_permission_changes_rejected() {
        for which in 0..3 {
            let f = Fixture::new();
            let held = ProtectedContext::load(&f.config).unwrap();
            let path = match which {
                0 => &f.config,
                1 => &f.socket,
                _ => f.dir.path(),
            };
            fs::set_permissions(path, fs::Permissions::from_mode(0o777)).unwrap();
            assert!(held.revalidate().is_err());
            assert!(ProtectedContext::load(&f.config).is_err());
        }
    }
    #[test]
    fn symlink_and_hardlink_config_refused() {
        let f = Fixture::new();
        let link = f.dir.path().join("link");
        symlink(&f.config, &link).unwrap();
        assert!(ProtectedContext::load(&link).is_err());
        fs::remove_file(&link).unwrap();
        fs::hard_link(&f.config, &link).unwrap();
        assert!(ProtectedContext::load(&f.config).is_err());
    }
    #[test]
    fn socket_symlink_and_regular_file_refused() {
        for link in [false, true] {
            let f = Fixture::new();
            let held = ProtectedContext::load(&f.config).unwrap();
            let old = f.dir.path().join("original.sock");
            fs::rename(&f.socket, &old).unwrap();
            if link {
                symlink(&old, &f.socket).unwrap();
            } else {
                fs::write(&f.socket, b"not a socket").unwrap();
            }
            assert!(held.revalidate().is_err());
            assert!(ProtectedContext::load(&f.config).is_err());
        }
    }
    #[test]
    fn changed_bytes_wrong_principal_metadata_and_duplicates_refused() {
        for which in 0..4 {
            let f = Fixture::new();
            let held = ProtectedContext::load(&f.config).unwrap();
            let mut v: serde_json::Value =
                serde_json::from_slice(&fs::read(&f.config).unwrap()).unwrap();
            let bytes = match which {
                0 => {
                    v["principal_uid"] = (u64::from(unsafe { libc::geteuid() }) + 1).into();
                    serde_json::to_vec(&v).unwrap()
                }
                1 => {
                    v["observation"]["socket"]["inode"] = 0.into();
                    serde_json::to_vec(&v).unwrap()
                }
                2 => b"{}".to_vec(),
                _ => serde_json::to_string(&v)
                    .unwrap()
                    .replacen(
                        "\"operator_epoch\":",
                        "\"operator_epoch\":\"bad\",\"operator_epoch\":",
                        1,
                    )
                    .into_bytes(),
            };
            fs::write(&f.config, bytes).unwrap();
            assert!(held.revalidate().is_err());
            assert!(ProtectedContext::load(&f.config).is_err());
        }
    }
}
