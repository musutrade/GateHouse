//! Linux filesystem checkpoint only: protected config bytes and held socket inode.
//! An O_PATH descriptor is not a connected transport or peer authentication.
use super::{
    native_json,
    observation::{ExpectedContext, SocketObservation},
};
use sha2::{Digest, Sha256};
use std::os::{
    fd::{AsRawFd, FromRawFd},
    unix::{
        ffi::OsStrExt,
        fs::{FileExt, MetadataExt, OpenOptionsExt},
    },
};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, &'static str>;
fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|_| "protected Podman filesystem rejected")
}
#[derive(PartialEq, Eq)]
pub(super) struct Identity {
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
    pub(super) fn of(m: &fs::Metadata) -> Self {
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
pub(super) fn protected(path: &Path) -> Result<()> {
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
        let file = if socket {
            // musl O_ACCMODE includes O_PATH; std OpenOptions masks it out.
            // Open the path-only handle directly, retaining atomic CLOEXEC/no-follow.
            let path = std::ffi::CString::new(path.as_os_str().as_bytes())
                .map_err(|_| "invalid Podman socket path")?;
            let fd = unsafe { libc::open(path.as_ptr(), flags) };
            if fd < 0 {
                return Err("protected Podman filesystem rejected");
            }
            // SAFETY: open returned a fresh owned descriptor.
            unsafe { File::from_raw_fd(fd) }
        } else {
            io(OpenOptions::new().read(true).custom_flags(flags).open(path))?
        };
        Self::from_file(path, file, socket)
    }
    fn from_file(path: &Path, file: File, socket: bool) -> Result<Self> {
        protected(path)?;
        if !socket {
            let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
            let fd_flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
            if flags < 0
                || flags & libc::O_ACCMODE != libc::O_RDONLY
                || flags & libc::O_PATH != 0
                || fd_flags < 0
                || fd_flags & libc::FD_CLOEXEC == 0
            {
                return Err("read-only cloexec config descriptor required");
            }
        }
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
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionedContext {
    schema: String,
    context: ExpectedContext,
}
pub const CONTEXT_SCHEMA: &str = "gate-runtime/podman-context-v1";
pub struct ProtectedContext {
    config: Held,
    socket: Held,
    bytes: Vec<u8>,
    context: ExpectedContext,
    versioned: bool,
}
impl ProtectedContext {
    /// Legacy development raw context only; never inferred as versioned.
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_format(path, false)
    }
    /// Canonical versioned context, held from the original open. Exact wrapper
    /// bytes are the snapshot and config digest, not reserialized inner bytes.
    pub fn load_versioned(path: &Path) -> Result<Self> {
        Self::load_format(path, true)
    }
    /// Takes ownership of the original read-only CLOEXEC config descriptor.
    /// Path is checked for identity only, never reopened for config bytes.
    /// Reads at explicit offsets, independent of the caller's shared cursor.
    pub fn from_held_versioned(path: &Path, file: File) -> Result<Self> {
        Self::from_config(Held::from_file(path, file, false)?, true)
    }
    pub fn is_versioned(&self) -> bool {
        self.versioned
    }
    fn load_format(path: &Path, versioned: bool) -> Result<Self> {
        Self::from_config(Held::open(path, false)?, versioned)
    }
    fn from_config(config: Held, versioned: bool) -> Result<Self> {
        let mut bytes = Vec::new();
        let mut buffer = [0; 8192];
        while bytes.len() <= native_json::LIMIT {
            let count = io(config.file.read_at(&mut buffer, bytes.len() as u64))?;
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        config.check()?;
        let value = native_json::decode(&bytes)?;
        let context: ExpectedContext = if versioned {
            let wrapper: VersionedContext =
                serde_json::from_value(value).map_err(|_| "versioned context fields rejected")?;
            if wrapper.schema != CONTEXT_SCHEMA
                || serde_json::to_vec(&wrapper).map_err(|_| "context encode")? != bytes
            {
                return Err("noncanonical versioned context");
            }
            wrapper.context
        } else {
            serde_json::from_value(value).map_err(|_| "protected context schema rejected")?
        };
        context.validate()?;
        context.validate_local_socket_policy()?;
        if context.principal_uid != unsafe { libc::geteuid() } {
            return Err("configured principal differs from euid");
        }
        let socket = Held::open(Path::new(&context.observation.socket.path), true)?;
        let held = Self {
            config,
            socket,
            bytes,
            context,
            versioned,
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
    /// Borrow the held Linux O_PATH socket handle after revalidation. A transport
    /// may connect through /proc/self/fd/N to avoid reopening the ambient path.
    /// This alone is not peer authentication or a mutation capability.
    pub fn socket_file(&self) -> Result<&File> {
        self.revalidate()?;
        Ok(&self.socket.file)
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
            let value = serde_json::json!({"operator_epoch":"a".repeat(32),"principal_uid":unsafe{libc::geteuid()},"observation":{"engine":"podman","api_surface":"libpod","selected_api":"6.1.4","server_version":"6.1.4","hostname":"fixture","rootless":unsafe{libc::geteuid()}!=0,"graph_root":"/var/lib/containers/storage","run_root":"/run/containers/storage","remote_socket":null,"socket":{"path":socket,"device":m.dev(),"inode":m.ino(),"uid":m.uid(),"mode":m.mode()}}});
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
    fn socket_handle_preserves_path_only_and_cloexec() {
        let f = Fixture::new();
        let held = Held::open(&f.socket, true).unwrap();
        let fd = held.file.as_raw_fd();
        assert_ne!(unsafe { libc::fcntl(fd, libc::F_GETFL) } & libc::O_PATH, 0);
        assert_ne!(
            unsafe { libc::fcntl(fd, libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        let mut byte = 0u8;
        assert_eq!(
            unsafe { libc::read(fd, (&mut byte as *mut u8).cast(), 1) },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EBADF)
        );
        assert_eq!(
            held.file.metadata().unwrap().ino(),
            fs::metadata(&f.socket).unwrap().ino()
        );
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
    fn info(held: &ProtectedContext) -> Vec<u8> {
        let (c, _) = held.snapshot().unwrap();
        let o = &c.observation;
        serde_json::to_vec(&serde_json::json!({"host":{"hostname":o.hostname,"security":{"rootless":o.rootless}},"store":{"graphRoot":o.graph_root,"runRoot":o.run_root},"version":{"APIVersion":"6.1.4","Version":"6.1.4"}})).unwrap()
    }
    fn absence(id: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"cause":"no such container","message":format!("no container with name or ID \"{id}\" found: no such container"),"response":404})).unwrap()
    }
    #[test]
    fn readonly_sequence_releases_result_only_after_final_info() {
        let f = Fixture::new();
        let held = ProtectedContext::load(&f.config).unwrap();
        let body = info(&held);
        let id = "a".repeat(64);
        let mut routes = vec![];
        let result = held
            .inspect_with(&id, |route| {
                routes.push(route.to_owned());
                Ok(if routes.len() == 2 {
                    (404, absence(&id))
                } else {
                    (200, body.clone())
                })
            })
            .unwrap();
        assert_eq!(result, super::super::native_status::InspectOutcome::Missing);
        assert_eq!(
            routes,
            vec![
                super::super::observation::INFO_ROUTE.to_string(),
                super::super::native_status::inspect_route(&id).unwrap(),
                super::super::observation::INFO_ROUTE.to_string()
            ]
        );
    }
    #[test]
    fn readonly_info_failure_and_invalid_id_do_not_inspect() {
        let f = Fixture::new();
        let held = ProtectedContext::load(&f.config).unwrap();
        let mut calls = 0;
        assert!(held
            .inspect_with("name", |_| {
                calls += 1;
                Ok((200, vec![]))
            })
            .is_err());
        assert_eq!(calls, 0);
        for status in [200, 401, 500] {
            calls = 0;
            assert!(held
                .inspect_with(&"a".repeat(64), |_| {
                    calls += 1;
                    Ok((status, b"{}".to_vec()))
                })
                .is_err());
            assert_eq!(calls, 1);
        }
    }
    #[test]
    fn readonly_final_failure_or_observation_change_withholds_absence() {
        for change in 0..3 {
            let f = Fixture::new();
            let held = ProtectedContext::load(&f.config).unwrap();
            let body = info(&held);
            let id = "a".repeat(64);
            let mut calls = 0;
            assert!(held
                .inspect_with(&id, |_| {
                    calls += 1;
                    match calls {
                        1 => Ok((200, body.clone())),
                        2 => Ok((404, absence(&id))),
                        _ => match change {
                            0 => Err("timeout"),
                            1 => Ok((500, body.clone())),
                            _ => {
                                let mut v: serde_json::Value =
                                    serde_json::from_slice(&body).unwrap();
                                v["host"]["hostname"] = "other".into();
                                Ok((200, serde_json::to_vec(&v).unwrap()))
                            }
                        },
                    }
                })
                .is_err());
            assert_eq!(calls, 3);
        }
    }
    #[test]
    fn readonly_revalidates_after_each_exchange_without_retry() {
        for boundary in 1..=3 {
            let f = Fixture::new();
            let held = ProtectedContext::load(&f.config).unwrap();
            let body = info(&held);
            let id = "a".repeat(64);
            let mut calls = 0;
            assert!(held
                .inspect_with(&id, |_| {
                    calls += 1;
                    if calls == boundary {
                        fs::set_permissions(&f.config, fs::Permissions::from_mode(0o644)).unwrap();
                    }
                    Ok(if calls == 2 {
                        (404, absence(&id))
                    } else {
                        (200, body.clone())
                    })
                })
                .is_err());
            assert_eq!(calls, boundary);
        }
    }
    #[test]
    fn readonly_initial_observation_mismatch_stops_before_inspect() {
        let f = Fixture::new();
        let held = ProtectedContext::load(&f.config).unwrap();
        let mut v: serde_json::Value = serde_json::from_slice(&info(&held)).unwrap();
        v["store"]["runRoot"] = "/run/other".into();
        let mut calls = 0;
        assert!(held
            .inspect_with(&"a".repeat(64), |_| {
                calls += 1;
                Ok((200, serde_json::to_vec(&v).unwrap()))
            })
            .is_err());
        assert_eq!(calls, 1);
    }
    #[test]
    fn readonly_socket_change_at_each_boundary_rejects_result() {
        for boundary in 1..=3 {
            let f = Fixture::new();
            let held = ProtectedContext::load(&f.config).unwrap();
            let body = info(&held);
            let id = "a".repeat(64);
            let mut calls = 0;
            assert!(held
                .inspect_with(&id, |_| {
                    calls += 1;
                    if calls == boundary {
                        fs::set_permissions(&f.socket, fs::Permissions::from_mode(0o660)).unwrap();
                    }
                    Ok(if calls == 2 {
                        (404, absence(&id))
                    } else {
                        (200, body.clone())
                    })
                })
                .is_err());
            assert_eq!(calls, boundary);
        }
    }
    #[test]
    fn readonly_inspect_failure_does_not_retry() {
        let f = Fixture::new();
        let held = ProtectedContext::load(&f.config).unwrap();
        let body = info(&held);
        let mut calls = 0;
        assert!(held
            .inspect_with(&"a".repeat(64), |_| {
                calls += 1;
                if calls == 1 {
                    Ok((200, body.clone()))
                } else {
                    Err("cancelled")
                }
            })
            .is_err());
        assert_eq!(calls, 2);
    }
    #[test]
    fn readonly_present_is_also_finally_checked() {
        let f = Fixture::new();
        let held = ProtectedContext::load(&f.config).unwrap();
        let body = info(&held);
        let id = "a".repeat(64);
        let mut calls = 0;
        let result=held.inspect_with(&id, |_|{calls+=1;Ok((200,if calls==2{serde_json::to_vec(&serde_json::json!({"Id":id,"State":{"Status":"initialized","Running":false,"Paused":false,"Restarting":false,"Dead":false,"ExitCode":0}})).unwrap()}else{body.clone()}))}).unwrap();
        assert_eq!(calls, 3);
        assert!(
            matches!(result,super::super::native_status::InspectOutcome::Present(state) if state.status=="initialized")
        );
    }
    #[test]
    fn ownership_composition_binds_body_and_final_observation() {
        use super::super::ownership::ExpectedContainer;
        use std::collections::BTreeMap;
        for failure in 0..3 {
            let f = Fixture::new();
            let held = ProtectedContext::load(&f.config).unwrap();
            let info_body = info(&held);
            let owner = ExpectedContainer {
                id: "a".repeat(64),
                name: "fixture".into(),
                image_id: "b".repeat(64),
                labels: BTreeMap::from([("lease".into(), "owned".into())]),
                port: None,
                image_exposed_ports: Default::default(),
            };
            let mut object = serde_json::json!({"Id":owner.id,"Name":owner.name,"Image":owner.image_id,"Config":{"Labels":owner.labels},"HostConfig":{"PortBindings":{},"Privileged":false,"AutoRemove":false,"AutoRemoveImage":false,"RestartPolicy":{"Name":"no","MaximumRetryCount":0}},"NetworkSettings":{"Ports":{}},"State":{"Status":"running","Running":true,"Paused":false,"Restarting":false,"Dead":false,"ExitCode":0}});
            if failure == 1 {
                object["Config"]["Labels"]["lease"] = "foreign".into();
            }
            let mut calls = 0;
            let result = held.inspect_owned_with(&owner, |_| {
                calls += 1;
                if failure == 2 && calls == 3 {
                    return Err("timeout");
                }
                Ok((
                    200,
                    if calls == 2 {
                        serde_json::to_vec(&object).unwrap()
                    } else {
                        info_body.clone()
                    },
                ))
            });
            if failure == 0 {
                assert_eq!(result.unwrap().status, "running");
                assert_eq!(calls, 3);
            } else {
                assert!(result.is_err());
                assert_eq!(calls, if failure == 1 { 2 } else { 3 });
            }
        }
    }
    #[test]
    fn image_metadata_is_withheld_until_final_protected_observation() {
        for failure in 0..3 {
            let f = Fixture::new();
            let held = ProtectedContext::load(&f.config).unwrap();
            let info_body = info(&held);
            let id = "b".repeat(64);
            let mut routes = vec![];
            let image = serde_json::json!({"Id":id,"Labels":{"image":"value"},"Config":{"Labels":{"image":"value"},"ExposedPorts":{"80/tcp":{}}}});
            let result = held.capture_image_for_plan(&id, |route| {
                routes.push(route.to_string());
                if failure == 1 && routes.len() == 2 {
                    return Ok((404, b"{}".to_vec()));
                }
                if failure == 2 && routes.len() == 3 {
                    return Err("cancelled");
                }
                Ok((
                    200,
                    if routes.len() == 2 {
                        serde_json::to_vec(&image).unwrap()
                    } else {
                        info_body.clone()
                    },
                ))
            });
            if failure == 0 {
                let facts = result.unwrap();
                assert_eq!(facts.facts().id(), id);
                assert_eq!(routes.len(), 3);
            } else {
                assert!(result.is_err());
                assert_eq!(routes.len(), if failure == 1 { 2 } else { 3 });
            }
            assert_eq!(
                routes[1],
                super::super::native_image::image_route(&id).unwrap()
            );
        }
    }
    #[test]
    fn capture_retains_exact_bytes_and_refuses_another_context() {
        let f = Fixture::new();
        let held = ProtectedContext::load(&f.config).unwrap();
        let id = "b".repeat(64);
        let raw = format!("{{ \"Id\":\"{id}\",\"Config\":{{}} }}").into_bytes();
        let info = info(&held);
        let mut calls = 0;
        let captured = held
            .capture_image_for_plan(&id, |_| {
                calls += 1;
                Ok((
                    200,
                    if calls == 2 {
                        raw.clone()
                    } else {
                        info.clone()
                    },
                ))
            })
            .unwrap();
        assert_eq!(calls, 3);
        assert_eq!(captured.image_bytes(), raw);
        assert_eq!(captured.config_bytes(), held.snapshot().unwrap().1);
        assert_eq!(
            captured.operator_epoch(),
            held.snapshot().unwrap().0.operator_epoch
        );
        assert_eq!(captured.facts().id(), id);
        captured.match_context(&held).unwrap();
        use super::super::{
            plan_record::{PlanInputs, PlanRecord},
            profile::{Profile, SlotIdentity, PRODUCT, PROTOCOL, VERSION},
        };
        use sha2::{Digest, Sha256};
        let identity = SlotIdentity {
            profile: Profile {
                protocol: PROTOCOL.into(),
                product: PRODUCT.into(),
                version: VERSION.into(),
                engine: "podman".into(),
            },
            installation_sha256: "a".repeat(64),
            executable_sha256: "c".repeat(64),
            config_sha256: format!("{:x}", Sha256::digest(captured.config_bytes())),
        };
        let inputs = PlanInputs {
            name: "fixture".into(),
            labels: [("owner".into(), "fixture".into())].into(),
            port: None,
        };
        let record = PlanRecord::capture(&held, &captured, &identity, inputs.clone()).unwrap();
        let bytes = record.encode().unwrap();
        let reopened = PlanRecord::decode(&bytes, &held).unwrap();
        let substituted = f.config.parent().unwrap().join("substituted-record");
        let replacement = String::from_utf8(bytes.clone())
            .unwrap()
            .replace(&"a".repeat(64), &"f".repeat(64));
        // This is independently valid evidence, so only exact intended-byte
        // comparison rejects substitution, not ordinary record validation.
        PlanRecord::decode(replacement.as_bytes(), &held).unwrap();
        assert_eq!(
            record
                .create_checked(&substituted, &held, || {
                    std::fs::write(&substituted, replacement.as_bytes()).unwrap();
                    Ok(())
                })
                .unwrap_err(),
            "created record differs from intended evidence"
        );
        assert_eq!(std::fs::read(&substituted).unwrap(), replacement.as_bytes());
        let disk = f.config.parent().unwrap().join("plan-record");
        record.create_new(&disk, &held).unwrap();
        assert_eq!(
            PlanRecord::open(&disk, &held).unwrap().encode().unwrap(),
            bytes
        );
        assert!(record.create_new(&disk, &held).is_err());
        std::fs::set_permissions(&disk, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(PlanRecord::open(&disk, &held).is_err());
        std::fs::set_permissions(&disk, std::fs::Permissions::from_mode(0o600)).unwrap();
        let linked = disk.with_extension("link");
        std::fs::hard_link(&disk, &linked).unwrap();
        assert!(PlanRecord::open(&disk, &held).is_err());
        std::fs::remove_file(&linked).unwrap();
        std::fs::write(&disk, b"{").unwrap();
        assert!(PlanRecord::open(&disk, &held).is_err());

        assert_eq!(reopened.plan_bytes(), record.plan_bytes());
        assert!(!reopened.installation_verified());
        assert_eq!(format!("{captured:?}"), "CapturedImage { .. }");
        let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for field in [
            "schema",
            "operator_epoch",
            "config_json",
            "config_sha256",
            "image_id",
            "image_json",
            "image_sha256",
            "plan_json",
            "plan_sha256",
        ] {
            let pattern = format!(
                "\"{field}\":{}",
                serde_json::to_string(&original[field]).unwrap()
            );
            let changed = String::from_utf8(bytes.clone()).unwrap().replacen(
                &pattern,
                &format!("\"{field}\":\"altered\""),
                1,
            );
            assert!(
                PlanRecord::decode(changed.as_bytes(), &held).is_err(),
                "{field}"
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(b' ');
        assert!(PlanRecord::decode(&trailing, &held).is_err());
        assert!(PlanRecord::decode(&bytes[..bytes.len() - 1], &held).is_err());
        let text = String::from_utf8(bytes.clone()).unwrap();
        let wrong_profile = text.replace("podman-lifecycle", "docker-lifecycle");
        assert!(PlanRecord::decode(wrong_profile.as_bytes(), &held).is_err());
        let wrong_inputs = text.replace("\"name\":\"fixture\"", "\"name\":\"other\"");
        assert_ne!(wrong_inputs, text);
        assert!(PlanRecord::decode(wrong_inputs.as_bytes(), &held).is_err());
        let duplicate_labels = text.replace("\"labels\":{", "\"labels\":{\"owner\":\"duplicate\",");
        assert!(PlanRecord::decode(duplicate_labels.as_bytes(), &held).is_err());
        let asserted_change = text.replace(&"a".repeat(64), &"f".repeat(64));
        assert!(!PlanRecord::decode(asserted_change.as_bytes(), &held)
            .unwrap()
            .installation_verified());
        let malformed_assertion = text.replace(&"a".repeat(64), "invalid");
        assert!(PlanRecord::decode(malformed_assertion.as_bytes(), &held).is_err());
        assert!(PlanRecord::decode(&vec![b' '; 16 * 1024 * 1024 + 1], &held).is_err());
        let duplicate = text.replacen("{", "{\"schema\":\"extra\",", 1);
        assert!(PlanRecord::decode(duplicate.as_bytes(), &held).is_err());
        let unknown = text.replacen("{", "{\"unknown\":0,", 1);
        assert!(PlanRecord::decode(unknown.as_bytes(), &held).is_err());
        let mut wrong_identity = identity.clone();
        wrong_identity.config_sha256 = "d".repeat(64);
        assert!(PlanRecord::capture(&held, &captured, &wrong_identity, inputs).is_err());

        let other = Fixture::new();
        assert!(
            PlanRecord::decode(&bytes, &ProtectedContext::load(&other.config).unwrap()).is_err()
        );
        assert!(captured
            .match_context(&ProtectedContext::load(&other.config).unwrap())
            .is_err());
    }
    fn evidence_fixture() -> (
        Fixture,
        ProtectedContext,
        super::super::plan_record::PlanRecord,
    ) {
        use super::super::{
            plan_record::{PlanInputs, PlanRecord},
            profile::{Profile, SlotIdentity, PRODUCT, PROTOCOL, VERSION},
        };
        let f = Fixture::new();
        let held = ProtectedContext::load(&f.config).unwrap();
        let id = "b".repeat(64);
        let raw = serde_json::to_vec(
            &serde_json::json!({"Id":id,"Config":{"ExposedPorts":{"80/tcp":{}}}}),
        )
        .unwrap();
        let info = info(&held);
        let mut calls = 0;
        let captured = held
            .capture_image_for_plan(&id, |_| {
                calls += 1;
                Ok((
                    200,
                    if calls == 2 {
                        raw.clone()
                    } else {
                        info.clone()
                    },
                ))
            })
            .unwrap();
        let identity = SlotIdentity {
            profile: Profile {
                protocol: PROTOCOL.into(),
                product: PRODUCT.into(),
                version: VERSION.into(),
                engine: "podman".into(),
            },
            installation_sha256: "a".repeat(64),
            executable_sha256: "c".repeat(64),
            config_sha256: held.config_sha256().unwrap(),
        };
        let record = PlanRecord::capture(
            &held,
            &captured,
            &identity,
            PlanInputs {
                name: "fixture".into(),
                labels: [("owner".into(), "fixture".into())].into(),
                port: None,
            },
        )
        .unwrap();
        (f, held, record)
    }
    fn replace_record_field(bytes: &[u8], field: &str, value: serde_json::Value) -> Vec<u8> {
        let original: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let old = format!(
            "\"{field}\":{}",
            serde_json::to_string(&original[field]).unwrap()
        );
        let new = format!("\"{field}\":{}", serde_json::to_string(&value).unwrap());
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains(&old));
        text.replacen(&old, &new, 1).into_bytes()
    }
    #[test]
    fn evidence_exposure_change_requires_hash_but_is_not_authenticity() {
        use super::super::plan_record::PlanRecord;
        let (_f, held, record) = evidence_fixture();
        let bytes = record.encode().unwrap();
        let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let image = original["image_json"]
            .as_str()
            .unwrap()
            .replace("80/tcp", "81/tcp");
        let changed = replace_record_field(&bytes, "image_json", serde_json::json!(image));
        assert!(PlanRecord::decode(&changed, &held).is_err());
        // Recomputed unkeyed hashes yield internally consistent evidence. They
        // cannot authenticate EXPOSE, which does not change native create body.
        let consistent = replace_record_field(
            &changed,
            "image_sha256",
            serde_json::json!(format!("{:x}", Sha256::digest(image.as_bytes()))),
        );
        let reopened = PlanRecord::decode(&consistent, &held).unwrap();
        assert_eq!(reopened.plan_bytes(), record.plan_bytes());
        assert!(!reopened.installation_verified());
    }
    #[test]
    fn evidence_nested_schema_and_decoded_bounds_refuse() {
        use super::super::plan_record::PlanRecord;
        let (_f, held, record) = evidence_fixture();
        let bytes = record.encode().unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        for (needle, replacement) in [
            ("\"profile\":{", "\"profile\":{\"engine\":\"podman\","),
            ("\"inputs\":{", "\"inputs\":{\"extra\":true,"),
            ("\"labels\":{", "\"labels\":{\"owner\":\"duplicate\","),
        ] {
            let altered = text.replacen(needle, replacement, 1);
            assert_ne!(altered, text);
            assert!(PlanRecord::decode(altered.as_bytes(), &held).is_err());
        }
        for field in ["config_json", "image_json"] {
            let altered = replace_record_field(
                &bytes,
                field,
                serde_json::json!(" ".repeat(1024 * 1024 + 1)),
            );
            assert!(PlanRecord::decode(&altered, &held).is_err());
        }
        let oversized_label = text.replacen(
            "\"owner\":\"fixture\"",
            &format!("\"owner\":\"{}\"", "x".repeat(4097)),
            1,
        );
        assert_ne!(oversized_label, text);
        assert!(PlanRecord::decode(oversized_label.as_bytes(), &held).is_err());
    }
    #[test]
    fn evidence_postcheck_replacement_remains_possible_and_non_authoritative() {
        use super::super::plan_record::PlanRecord;
        let (f, held, record) = evidence_fixture();
        let path = f.config.parent().unwrap().join("postcheck");
        record.create_new(&path, &held).unwrap();
        let intended = record.encode().unwrap();
        let replacement = replace_record_field(
            &intended,
            "asserted_installation_sha256",
            serde_json::json!("f".repeat(64)),
        );
        // Deterministic ordering after final return, no sleep/race timing.
        fs::write(&path, &replacement).unwrap();
        let reopened = PlanRecord::open(&path, &held).unwrap();
        assert_ne!(reopened.encode().unwrap(), intended);
        assert!(!reopened.installation_verified());
    }
    #[test]
    fn versioned_context_retains_wrapper_and_refuses_raw_record_adoption() {
        use super::super::plan_record::PlanRecord;
        let (f, raw, record) = evidence_fixture();
        let old_bytes = record.encode().unwrap();
        assert!(!raw.is_versioned());
        assert!(ProtectedContext::load_versioned(&f.config).is_err());
        let wrapper = VersionedContext {
            schema: CONTEXT_SCHEMA.into(),
            context: raw.snapshot().unwrap().0.clone(),
        };
        let bytes = serde_json::to_vec(&wrapper).unwrap();
        fs::write(&f.config, &bytes).unwrap();
        assert!(raw.revalidate().is_err());
        assert!(ProtectedContext::load(&f.config).is_err());
        let held = ProtectedContext::load_versioned(&f.config).unwrap();
        assert!(held.is_versioned());
        assert_eq!(held.snapshot().unwrap().1, bytes);
        assert_eq!(
            held.config_sha256().unwrap(),
            format!("{:x}", Sha256::digest(&bytes))
        );
        assert!(PlanRecord::decode(&old_bytes, &held).is_err());
        let info = info(&held);
        let mut calls = 0;
        let captured = held
            .capture_image_for_plan(&"b".repeat(64), |_| {
                calls += 1;
                Ok((
                    200,
                    if calls == 2 {
                        serde_json::to_vec(&serde_json::json!({"Id":"b".repeat(64),"Config":{}}))
                            .unwrap()
                    } else {
                        info.clone()
                    },
                ))
            })
            .unwrap();
        assert_eq!(captured.config_bytes(), bytes);
    }
    #[test]
    fn versioned_context_rejects_schema_duplicate_unknown_and_whitespace() {
        let f = Fixture::new();
        let raw = ProtectedContext::load(&f.config).unwrap();
        let wrapper = VersionedContext {
            schema: CONTEXT_SCHEMA.into(),
            context: raw.snapshot().unwrap().0.clone(),
        };
        let text = serde_json::to_string(&wrapper).unwrap();
        for bad in [
            text.replace(CONTEXT_SCHEMA, "other"),
            format!("{text} "),
            text.replacen("{", "{\"extra\":0,", 1),
            text.replacen("{", "{\"schema\":\"duplicate\",", 1),
        ] {
            fs::write(&f.config, bad).unwrap();
            assert!(ProtectedContext::load_versioned(&f.config).is_err());
        }
    }
    #[test]
    fn held_versioned_cursor_independence_and_record_roundtrip() {
        use super::super::{
            plan_record::{PlanInputs, PlanRecord},
            profile::{Profile, SlotIdentity, PRODUCT, PROTOCOL, VERSION},
        };
        use std::io::{Seek, SeekFrom};
        let f = Fixture::new();
        let raw = ProtectedContext::load(&f.config).unwrap();
        let bytes = serde_json::to_vec(&VersionedContext {
            schema: CONTEXT_SCHEMA.into(),
            context: raw.snapshot().unwrap().0.clone(),
        })
        .unwrap();
        fs::write(&f.config, &bytes).unwrap();
        let mut file = File::open(&f.config).unwrap();
        file.seek(SeekFrom::End(0)).unwrap();
        let held = ProtectedContext::from_held_versioned(&f.config, file).unwrap();
        assert_eq!(held.snapshot().unwrap().1, bytes);
        let info = info(&held);
        let mut calls = 0;
        let id = "b".repeat(64);
        let captured = held
            .capture_image_for_plan(&id, |_| {
                calls += 1;
                Ok((
                    200,
                    if calls == 2 {
                        serde_json::to_vec(&serde_json::json!({"Id":id,"Config":{}})).unwrap()
                    } else {
                        info.clone()
                    },
                ))
            })
            .unwrap();
        let identity = SlotIdentity {
            profile: Profile {
                protocol: PROTOCOL.into(),
                product: PRODUCT.into(),
                version: VERSION.into(),
                engine: "podman".into(),
            },
            installation_sha256: "a".repeat(64),
            executable_sha256: "c".repeat(64),
            config_sha256: held.config_sha256().unwrap(),
        };
        let record = PlanRecord::capture(
            &held,
            &captured,
            &identity,
            PlanInputs {
                name: "fixture".into(),
                labels: [("owner".into(), "fixture".into())].into(),
                port: None,
            },
        )
        .unwrap();
        let path = f.dir.path().join("versioned-record");
        record.create_new(&path, &held).unwrap();
        let reopened = PlanRecord::open(&path, &held).unwrap();
        assert_eq!(reopened.encode().unwrap(), record.encode().unwrap());
        assert!(!reopened.installation_verified());
    }
    #[test]
    fn held_versioned_rejects_wrong_inode_replaced_path_and_writable_descriptor() {
        let f = Fixture::new();
        let raw = ProtectedContext::load(&f.config).unwrap();
        let bytes = serde_json::to_vec(&VersionedContext {
            schema: CONTEXT_SCHEMA.into(),
            context: raw.snapshot().unwrap().0.clone(),
        })
        .unwrap();
        fs::write(&f.config, &bytes).unwrap();
        let writable = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&f.config)
            .unwrap();
        assert!(ProtectedContext::from_held_versioned(&f.config, writable).is_err());
        let old = File::open(&f.config).unwrap();
        fs::rename(&f.config, f.dir.path().join("original")).unwrap();
        fs::write(&f.config, &bytes).unwrap();
        fs::set_permissions(&f.config, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(ProtectedContext::from_held_versioned(&f.config, old).is_err());
        let other = Fixture::new();
        assert!(ProtectedContext::from_held_versioned(
            &f.config,
            File::open(&other.config).unwrap()
        )
        .is_err());
    }
    fn installed_fixture() -> (Fixture, PathBuf) {
        let f = Fixture::new();
        let raw = ProtectedContext::load(&f.config).unwrap();
        let config = serde_json::to_vec(&VersionedContext {
            schema: CONTEXT_SCHEMA.into(),
            context: raw.snapshot().unwrap().0.clone(),
        })
        .unwrap();
        fs::write(&f.config, &config).unwrap();
        let exe = f.dir.path().join("adapter");
        fs::write(&exe, b"\x7fELFfake-no-execution").unwrap();
        fs::set_permissions(
            &exe,
            fs::Permissions::from_mode(if unsafe { libc::geteuid() } == 0 {
                0o755
            } else {
                0o700
            }),
        )
        .unwrap();
        let descriptor=format!("{{\"schema\":\"gate-runtime/podman-installation-v1\",\"product\":\"podman-lifecycle\",\"version\":\"0.1.0\",\"protocol\":\"gate-runtime/lifecycle-v2\",\"executable_path\":{},\"executable_sha256\":\"{:x}\",\"config_schema\":\"{}\"}}",serde_json::to_string(&exe).unwrap(),Sha256::digest(fs::read(&exe).unwrap()),CONTEXT_SCHEMA);
        let install = f.dir.path().join("installation.json");
        fs::write(&install, &descriptor).unwrap();
        fs::set_permissions(&install, fs::Permissions::from_mode(0o600)).unwrap();
        let host=format!("{{\"schema\":\"gate-runtime/podman-host-v1\",\"installation_path\":{},\"installation_sha256\":\"{:x}\",\"config_path\":{},\"config_sha256\":\"{:x}\"}}",serde_json::to_string(&install).unwrap(),Sha256::digest(descriptor.as_bytes()),serde_json::to_string(&f.config).unwrap(),Sha256::digest(&config));
        let path = f.dir.path().join("host.json");
        fs::write(&path, host).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        (f, path)
    }
    #[test]
    fn held_installation_record_match_revalidates_without_upgrading_record() {
        use super::super::{
            installation::VerifiedPodmanInstallation,
            plan_record::{PlanInputs, PlanRecord},
        };
        let (f, path) = installed_fixture();
        let installed = VerifiedPodmanInstallation::load(&path).unwrap();
        let context = installed.context().unwrap();
        assert!(context.is_versioned());
        let info = info(context);
        let mut calls = 0;
        let id = "b".repeat(64);
        let capture = context
            .capture_image_for_plan(&id, |_| {
                calls += 1;
                Ok((
                    200,
                    if calls == 2 {
                        serde_json::to_vec(&serde_json::json!({"Id":id,"Config":{}})).unwrap()
                    } else {
                        info.clone()
                    },
                ))
            })
            .unwrap();
        let inputs = super::super::plan_record::PlanInputs {
            name: "fixture".into(),
            labels: [("owner".into(), "fixture".into())].into(),
            port: None,
        };
        let record = PlanRecord::capture(
            context,
            &capture,
            installed.identity().unwrap(),
            inputs.clone(),
        )
        .unwrap();
        installed
            .match_record(&record)
            .unwrap()
            .revalidate()
            .unwrap();
        assert!(!record.installation_verified());
        let mut mismatch = installed.identity().unwrap().clone();
        mismatch.executable_sha256 = "f".repeat(64);
        let wrong =
            PlanRecord::capture(context, &capture, &mismatch, PlanInputs { ..inputs }).unwrap();
        assert!(installed.match_record(&wrong).is_err());
        let matched = installed.match_record(&record).unwrap();
        fs::write(f.dir.path().join("adapter"), b"\x7fELFreplaced").unwrap();
        assert!(matched.revalidate().is_err());
        assert!(!record.installation_verified());
    }
    #[test]
    fn installation_substitution_modes_schema_and_digests_refuse() {
        use super::super::installation::VerifiedPodmanInstallation;
        for case in 0..7 {
            let (f, path) = installed_fixture();
            match case {
                0 => fs::set_permissions(
                    f.dir.path().join("adapter"),
                    fs::Permissions::from_mode(0o777),
                )
                .unwrap(),
                1 => {
                    fs::write(f.dir.path().join("adapter"), b"not ELF").unwrap();
                }
                2 => {
                    fs::write(&f.config, b"{}").unwrap();
                }
                3 => {
                    let text = fs::read_to_string(&path)
                        .unwrap()
                        .replace("podman-host-v1", "docker-host-v1");
                    fs::write(&path, text).unwrap();
                }
                4 => {
                    fs::hard_link(f.dir.path().join("adapter"), f.dir.path().join("alias"))
                        .unwrap();
                }
                5 => {
                    let text = fs::read_to_string(&path).unwrap();
                    fs::write(&path, format!("{text} ")).unwrap();
                }
                6 => {
                    let exe = f.dir.path().join("adapter");
                    fs::rename(&exe, f.dir.path().join("real-adapter")).unwrap();
                    std::os::unix::fs::symlink(f.dir.path().join("real-adapter"), &exe).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                VerifiedPodmanInstallation::load(&path).is_err(),
                "case {case}"
            );
        }
    }
    #[test]
    fn held_config_rejects_opath_no_cloexec_hardlink_and_symlink() {
        for case in 0..4 {
            let (f, _) = installed_fixture();
            let file = if case == 0 {
                let path = std::ffi::CString::new(f.config.as_os_str().as_bytes()).unwrap();
                let fd = unsafe { libc::open(path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
                assert!(fd >= 0);
                assert_ne!(unsafe { libc::fcntl(fd, libc::F_GETFL) } & libc::O_PATH, 0);
                unsafe { File::from_raw_fd(fd) }
            } else {
                File::open(&f.config).unwrap()
            };
            if case == 1 {
                assert_eq!(
                    unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, 0) },
                    0
                );
            }
            if case == 2 {
                fs::hard_link(&f.config, f.dir.path().join("config-link")).unwrap();
            }
            if case == 3 {
                fs::rename(&f.config, f.dir.path().join("config-original")).unwrap();
                std::os::unix::fs::symlink(f.dir.path().join("config-original"), &f.config)
                    .unwrap();
            }
            assert!(ProtectedContext::from_held_versioned(&f.config, file).is_err());
        }
    }
    #[test]
    fn every_held_installation_member_replacement_is_rejected() {
        use super::super::installation::VerifiedPodmanInstallation;
        for name in ["host.json", "installation.json", "context.json", "adapter"] {
            let (f, path) = installed_fixture();
            let installed = VerifiedPodmanInstallation::load(&path).unwrap();
            let member = f.dir.path().join(name);
            let bytes = fs::read(&member).unwrap();
            let mode = fs::metadata(&member).unwrap().permissions();
            fs::rename(&member, f.dir.path().join("old-member")).unwrap();
            fs::write(&member, bytes).unwrap();
            fs::set_permissions(&member, mode).unwrap();
            assert!(installed.revalidate().is_err(), "{name}");
        }
    }
    #[test]
    fn capture_request_matches_all_operator_fields_and_exact_image() {
        use super::super::plan_record::{PlanInputs, PlanPort};
        let (_f, _held, record) = evidence_fixture();
        let inputs = PlanInputs {
            name: "fixture".into(),
            labels: [("owner".into(), "fixture".into())].into(),
            port: None,
        };
        let id = "b".repeat(64);
        record.match_capture_request(&id, &inputs).unwrap();
        assert!(record
            .match_capture_request(&"c".repeat(64), &inputs)
            .is_err());
        for case in 0..4 {
            let mut changed = inputs.clone();
            match case {
                0 => changed.name = "another".into(),
                1 => {
                    changed.labels.insert("owner".into(), "another".into());
                }
                2 => changed.labels.clear(),
                3 => {
                    changed.port = Some(PlanPort {
                        container: 80,
                        host: 8080,
                    })
                }
                _ => unreachable!(),
            };
            assert!(record.match_capture_request(&id, &changed).is_err());
        }
    }
    #[test]
    fn capture_wire_roundtrip_and_binding_negatives() {
        use super::super::{
            capture_wire::*,
            plan_record::PlanInputs,
            profile::{Profile, SlotIdentity, PRODUCT, PROTOCOL, VERSION},
        };
        let (_f, held, record) = evidence_fixture();
        let profile = Profile {
            protocol: PROTOCOL.into(),
            product: PRODUCT.into(),
            version: VERSION.into(),
            engine: "podman".into(),
        };
        let request = CaptureRequest {
            schema: "gate-runtime/podman-capture-v1".into(),
            profile: profile.clone(),
            invocation: "1".repeat(32),
            image_id: "b".repeat(64),
            inputs: PlanInputs {
                name: "fixture".into(),
                labels: [("owner".into(), "fixture".into())].into(),
                port: None,
            },
            timeout_ms: 1000,
        };
        let identity = SlotIdentity {
            profile,
            installation_sha256: "a".repeat(64),
            config_sha256: held.config_sha256().unwrap(),
            executable_sha256: "c".repeat(64),
        };
        let expected = CaptureExpected::bind(
            &request,
            &identity,
            "d".repeat(64),
            held.config.path.to_str().unwrap().into(),
        )
        .unwrap();
        let rbytes = request.encode().unwrap();
        CaptureRequest::decode(&rbytes).unwrap();
        let ebytes = expected.encode(&request).unwrap();
        CaptureExpected::decode(&ebytes, &request).unwrap();
        let response = encode_response(&record, &request, &expected, &held).unwrap();
        assert_eq!(
            decode_response(&response, &request, &expected, &held)
                .unwrap()
                .encode()
                .unwrap(),
            record.encode().unwrap()
        );
        for field in [
            "schema",
            "invocation",
            "operation_key",
            "request_sha256",
            "record_b64",
            "record_sha256",
        ] {
            let bad = replace_record_field(&response, field, serde_json::json!("invalid"));
            assert!(
                decode_response(&bad, &request, &expected, &held).is_err(),
                "{field}"
            );
        }
        for field in [
            "schema",
            "invocation",
            "request_sha256",
            "host_sha256",
            "installation_sha256",
            "config_sha256",
            "executable_sha256",
            "config_path",
            "operation_key",
        ] {
            let bad = replace_record_field(&ebytes, field, serde_json::json!("invalid"));
            assert!(CaptureExpected::decode(&bad, &request).is_err(), "{field}");
        }
        for (field, value) in [
            ("schema", serde_json::json!("gate-runtime/lifecycle-v2")),
            ("invocation", serde_json::json!("BAD")),
            ("timeout_ms", serde_json::json!(0)),
            ("timeout_ms", serde_json::json!(30001)),
            ("image_id", serde_json::json!("short")),
        ] {
            assert!(CaptureRequest::decode(&replace_record_field(&rbytes, field, value)).is_err());
        }
        let duplicate = String::from_utf8(rbytes.clone()).unwrap().replacen(
            "{",
            "{\"schema\":\"duplicate\",",
            1,
        );
        assert!(CaptureRequest::decode(duplicate.as_bytes()).is_err());
        let mut trailing = response.clone();
        trailing.push(b'\n');
        assert!(decode_response(&trailing, &request, &expected, &held).is_err());
        assert!(CaptureRequest::decode(&vec![b' '; 65537]).is_err());
        assert!(CaptureExpected::decode(&vec![b' '; 16385], &request).is_err());
        assert!(decode_response(
            &vec![b' '; 24 * 1024 * 1024 + 1],
            &request,
            &expected,
            &held
        )
        .is_err());
    }
    #[test]
    fn protected_capture_inputs_bind_original_fd_and_restore_cloexec() {
        use super::super::{
            capture_inputs,
            capture_wire::{CaptureExpected, CaptureRequest},
            installation::VerifiedPodmanInstallation,
            plan_record::PlanInputs,
        };
        let (f, host) = installed_fixture();
        let installed = VerifiedPodmanInstallation::load(&host).unwrap();
        let request = CaptureRequest {
            schema: "gate-runtime/podman-capture-v1".into(),
            profile: installed.identity().unwrap().profile.clone(),
            invocation: "1".repeat(32),
            image_id: "b".repeat(64),
            inputs: PlanInputs {
                name: "fixture".into(),
                labels: [("owner".into(), "fixture".into())].into(),
                port: None,
            },
            timeout_ms: 1000,
        };
        let expected = CaptureExpected::bind(
            &request,
            installed.identity().unwrap(),
            format!("{:x}", Sha256::digest(fs::read(&host).unwrap())),
            f.config.to_str().unwrap().into(),
        )
        .unwrap();
        let raw = request.encode().unwrap();
        let eb = expected.encode(&request).unwrap();
        let seals =
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
        let config = File::open(&f.config).unwrap();
        let witness = config.try_clone().unwrap();
        assert_eq!(
            unsafe { libc::fcntl(config.as_raw_fd(), libc::F_SETFD, 0) },
            0
        );
        let parsed =
            capture_inputs::parse(config, capture_inputs::test_memory(&eb, seals, 0o600), &raw)
                .unwrap();
        assert_eq!(
            parsed.context().unwrap().snapshot().unwrap().1,
            fs::read(&f.config).unwrap()
        );
        assert_ne!(
            unsafe {
                libc::fcntl(
                    parsed.context().unwrap().config.file.as_raw_fd(),
                    libc::F_GETFD,
                )
            } & libc::FD_CLOEXEC,
            0
        );
        assert_eq!(parsed.expected().operation_key, expected.operation_key);
        assert_eq!(parsed.request().invocation, request.invocation);
        assert_ne!(
            unsafe { libc::fcntl(witness.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        let other = Fixture::new();
        assert!(capture_inputs::parse(
            File::open(&other.config).unwrap(),
            capture_inputs::test_memory(&eb, seals, 0o600),
            &raw
        )
        .is_err());
        let altered = replace_record_field(&raw, "image_id", serde_json::json!("c".repeat(64)));
        assert!(capture_inputs::parse(
            File::open(&f.config).unwrap(),
            capture_inputs::test_memory(&eb, seals, 0o600),
            &altered
        )
        .is_err());
        assert!(capture_inputs::parse(
            File::open(&f.config).unwrap(),
            capture_inputs::test_memory(&eb, 0, 0o600),
            &raw
        )
        .is_err());
    }
}
