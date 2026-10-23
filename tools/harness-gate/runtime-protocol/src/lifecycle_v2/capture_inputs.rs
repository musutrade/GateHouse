//! Inherited descriptor validation only; seals are not launcher authentication.
use super::{
    capture_wire::{CaptureExpected, CaptureRequest},
    protected_context::ProtectedContext,
};
use std::{
    fs::File,
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, MetadataExt},
    },
    path::Path,
};
type Result<T> = std::result::Result<T, &'static str>;
const SEALS: i32 = libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
/// No public fields or constructor. Contains no dispatch or mutation capability.
pub struct ProtectedCaptureInputs {
    request: CaptureRequest,
    expected: CaptureExpected,
    context: ProtectedContext,
}
impl ProtectedCaptureInputs {
    pub fn request(&self) -> &CaptureRequest {
        &self.request
    }
    pub fn expected(&self) -> &CaptureExpected {
        &self.expected
    }
    pub fn context(&self) -> Result<&ProtectedContext> {
        self.context.revalidate()?;
        Ok(&self.context)
    }
}
/// Caller must transfer child-owned fd3/fd4, never borrowed parent descriptors.
/// Restores CLOEXEC on the consumed descriptors before further use; no dup/open
/// of the config path. Public same-UID callers can forge expected inputs.
pub fn parse(config: File, expected: File, stdin: &[u8]) -> Result<ProtectedCaptureInputs> {
    let request = CaptureRequest::decode(stdin)?;
    cloexec(&config)?;
    cloexec(&expected)?;
    let bytes = sealed_bytes(&expected)?;
    let expected = CaptureExpected::decode(&bytes, &request)?;
    let context = ProtectedContext::from_held_versioned(Path::new(&expected.config_path), config)?;
    if context.config_sha256()? != expected.config_sha256 {
        return Err("protected capture config mismatch");
    }
    context.revalidate()?;
    Ok(ProtectedCaptureInputs {
        request,
        expected,
        context,
    })
}
fn cloexec(file: &File) -> Result<()> {
    let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
    if flags < 0
        || unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0
    {
        return Err("capture descriptor flags rejected");
    }
    Ok(())
}
fn sealed_bytes(file: &File) -> Result<Vec<u8>> {
    let m = file.metadata().map_err(|_| "capture expected metadata")?;
    let seals = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GET_SEALS) };
    if !m.is_file()
        || m.nlink() != 0
        || m.mode() & 0o7777 != 0o600
        || m.uid() != unsafe { libc::geteuid() }
        || m.len() == 0
        || m.len() > 16384
        || seals < 0
        || seals & SEALS != SEALS
    {
        return Err("capture expected must be bounded private sealed memory");
    }
    let mut bytes = vec![0; m.len() as usize];
    let mut offset = 0;
    while offset < bytes.len() {
        match file.read_at(&mut bytes[offset..], offset as u64) {
            Ok(0) => return Err("capture expected short read"),
            Ok(n) => offset += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err("capture expected read failed"),
        }
    }
    Ok(bytes)
}
#[cfg(test)]
pub(super) fn test_memory(bytes: &[u8], seals: i32, mode: u32) -> File {
    use std::{io::Write, os::fd::FromRawFd};
    let fd = unsafe {
        libc::memfd_create(
            c"podman-capture-test".as_ptr(),
            libc::MFD_ALLOW_SEALING | libc::MFD_CLOEXEC,
        )
    };
    assert!(fd >= 0);
    let mut file = unsafe { File::from_raw_fd(fd) };
    assert_eq!(unsafe { libc::fchmod(fd, mode) }, 0);
    file.write_all(bytes).unwrap();
    if seals != 0 {
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, seals) }, 0);
    }
    file
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sealed_expected_shape_and_every_seal_required() {
        let good = test_memory(b"test", SEALS, 0o600);
        assert_eq!(sealed_bytes(&good).unwrap(), b"test");
        for seal in [
            libc::F_SEAL_WRITE,
            libc::F_SEAL_GROW,
            libc::F_SEAL_SHRINK,
            libc::F_SEAL_SEAL,
        ] {
            assert!(sealed_bytes(&test_memory(b"test", SEALS & !seal, 0o600)).is_err());
        }
        for (bytes, mode) in [
            (vec![], 0o600),
            (vec![b'x'; 16385], 0o600),
            (b"test".to_vec(), 0o644),
        ] {
            assert!(sealed_bytes(&test_memory(&bytes, SEALS, mode)).is_err());
        }
        let ordinary = tempfile::tempfile().unwrap();
        assert!(sealed_bytes(&ordinary).is_err());
    }
}
