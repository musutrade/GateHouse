//! Linux inherited input checks. Kernel seals establish immutability, not caller
//! authentication. Core must mint expected inputs from the verified installation
//! and a held journal dispatch; arbitrary same-UID launchers are not trusted.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, MetadataExt},
    },
};
const SEALS: i32 = libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
fn read(file: &File) -> Result<Vec<u8>> {
    let m = file
        .metadata()
        .map_err(|_| Invalid("invalid lifecycle protected descriptor"))?;
    // SAFETY: live borrowed file; fcntl only queries metadata.
    let seals = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GET_SEALS) };
    if !m.is_file()
        || m.nlink() != 0
        || m.mode() & 0o7777 != 0o600
        || m.uid() != unsafe { libc::geteuid() }
        || m.len() == 0
        || m.len() > crate::MAX_MESSAGE_BYTES as u64
        || seals < 0
        || seals & SEALS != SEALS
    {
        return Err(Invalid(
            "lifecycle input must be bounded private sealed memory",
        ));
    }
    let mut bytes = vec![0; m.len() as usize];
    let mut offset = 0;
    while offset < bytes.len() {
        match file.read_at(&mut bytes[offset..], offset as u64) {
            Ok(0) => return Err(Invalid("incomplete lifecycle protected input")),
            Ok(n) => offset += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Err(Invalid("cannot read lifecycle protected input")),
        }
    }
    Ok(bytes)
}
/// Adapter entry supplies inherited fd3/fd4 as borrowed files and bounded stdin.
/// No descriptor/path is opened from the environment and no engine I/O occurs.
pub fn parse_protected_inputs(config: &File, expected: &File, stdin: &[u8]) -> Result<Request> {
    if stdin.len() > crate::MAX_MESSAGE_BYTES {
        return Err(Invalid("lifecycle stdin too large"));
    }
    let config = read(config)?;
    let expected = read(expected)?;
    let request = super::super::wire::parse_protected_request(stdin, &expected)?;
    let c: Config = parse(&config)?;
    if c.schema != "gate-runtime/lifecycle-config-v1"
        || c.engine_binding != request.binding.engine_binding
        || request.binding.config_sha256 != format!("{:x}", Sha256::digest(&config))
        || c.engine_binding.principal_uid != unsafe { libc::geteuid() }
    {
        return Err(Invalid("lifecycle protected configuration mismatch"));
    }
    Ok(request)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, os::fd::FromRawFd};
    fn memory(bytes: &[u8], seal: bool, mode: u32) -> File {
        let fd = unsafe {
            libc::memfd_create(
                c"lifecycle-test-input".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
            )
        };
        assert!(fd >= 0);
        let mut f = unsafe { File::from_raw_fd(fd) };
        assert_eq!(unsafe { libc::fchmod(fd, mode) }, 0);
        f.write_all(bytes).unwrap();
        if seal {
            assert_eq!(unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, SEALS) }, 0);
        }
        f
    }
    fn fixture() -> (Vec<u8>, Vec<u8>) {
        let mut r =
            super::super::super::wire::parse_request(include_bytes!("../wire/status-request.json"))
                .unwrap();
        r.binding.engine_binding.principal_uid = unsafe { libc::geteuid() };
        let c = Config {
            schema: "gate-runtime/lifecycle-config-v1".into(),
            engine_binding: r.binding.engine_binding.clone(),
        };
        let config = canonical::encode(&serde_json::to_value(c).unwrap()).unwrap();
        r.binding.config_sha256 = format!("{:x}", Sha256::digest(&config));
        r.binding.operation_key = r.operation_key().unwrap();
        (config, r.canonical_bytes().unwrap())
    }
    #[test]
    fn exact_sealed_inputs_ignore_shared_cursor_and_reject_substitution() {
        let (c, r) = fixture();
        let config = memory(&c, true, 0o600);
        let expected = memory(&r, true, 0o600);
        assert!(parse_protected_inputs(&config, &expected, &r).is_ok());
        assert!(parse_protected_inputs(&config, &expected, &r).is_ok());
        assert!(parse_protected_inputs(&config, &expected, b"{}").is_err());
        let changed = memory(b"{}", true, 0o600);
        assert!(parse_protected_inputs(&changed, &expected, &r).is_err());
    }
    #[test]
    fn unsealed_public_empty_and_oversized_inputs_rejected() {
        let (c, r) = fixture();
        let config = memory(&c, true, 0o600);
        for expected in [
            memory(&r, false, 0o600),
            memory(&r, true, 0o644),
            memory(b"", true, 0o600),
            memory(&vec![b'x'; 65537], true, 0o600),
        ] {
            assert!(parse_protected_inputs(&config, &expected, &r).is_err());
        }
        let regular = tempfile::tempfile().unwrap();
        assert!(read(&regular).is_err());
    }
}
