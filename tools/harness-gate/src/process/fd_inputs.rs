//! Linux-only sealed bootstrap inputs. This is a descriptor transport primitive,
//! not request validation, installed-artifact authentication or an engine host.
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::Command;

const MAX_INPUT_BYTES: usize = 65_536;
const SEALS: libc::c_int =
    libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;

// Staged for the additive v2 host; existing v1 callers are unchanged.
#[allow(dead_code)]
pub(super) struct SealedInputs {
    config: OwnedFd,
    expected: OwnedFd,
}

#[allow(dead_code)]
impl SealedInputs {
    /// The caller must authenticate both byte strings before constructing inputs.
    pub(super) fn new(config: &[u8], expected: &[u8]) -> io::Result<Self> {
        if config.len() > MAX_INPUT_BYTES || expected.len() > MAX_INPUT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "runtime input too large",
            ));
        }
        Ok(Self {
            config: sealed(config)?,
            expected: sealed(expected)?,
        })
    }

    /// Consume inputs into the Command so their lifetime covers every spawn.
    /// Callers own stdio, environment, cwd, executable pins and supervision.
    /// Install once, with no subsequent pre_exec callback that changes fd policy.
    pub(super) fn install(self, command: &mut Command) -> io::Result<()> {
        // Stage both sources above reserved targets before any child remapping.
        // Reserve any vacant target numbers in the parent as well: otherwise
        // Command could allocate its private exec-error socket at fd3/fd4.
        let reserve_a = duplicate(&self.config, 3)?;
        let reserve_b = duplicate(&self.expected, 3)?;
        let config = duplicate(&self.config, 5)?;
        let expected = duplicate(&self.expected, 5)?;
        unsafe {
            // SAFETY: the child callback uses only raw syscalls and errno reads.
            // Owned descriptors are captured before fork and live with Command.
            command.pre_exec(move || {
                // Keep reservations owned by this closure until after spawn.
                let _reserved = (
                    self.config.as_raw_fd(),
                    self.expected.as_raw_fd(),
                    reserve_a.as_raw_fd(),
                    reserve_b.as_raw_fd(),
                );
                // Do not close Rust's internal exec-error socket: it must remain
                // usable on failure and close only when exec succeeds. No bounded
                // fd-number loop is sufficient to exclude high inherited fds.
                if libc::syscall(
                    libc::SYS_close_range,
                    5u32,
                    u32::MAX,
                    libc::CLOSE_RANGE_CLOEXEC,
                ) == -1
                {
                    return Err(io::Error::last_os_error());
                }
                if libc::dup2(config.as_raw_fd(), 3) == -1
                    || libc::dup2(expected.as_raw_fd(), 4) == -1
                {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        Ok(())
    }
}

fn duplicate(fd: &OwnedFd, minimum: libc::c_int) -> io::Result<OwnedFd> {
    let result = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, minimum) };
    if result == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fcntl returned a new descriptor exclusively owned here.
    Ok(unsafe { OwnedFd::from_raw_fd(result) })
}

fn sealed(bytes: &[u8]) -> io::Result<OwnedFd> {
    let fd = unsafe {
        libc::memfd_create(
            c"gate-runtime-input".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if fd == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: memfd_create returned a new descriptor exclusively owned here.
    let mut file = unsafe { File::from_raw_fd(fd) };
    if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } == -1 {
        return Err(io::Error::last_os_error());
    }
    file.write_all(bytes)?;
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_ADD_SEALS, SEALS) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(file.into())
}
