//! Linux-only sealed bootstrap inputs. This is a descriptor transport primitive,
//! not request validation, installed-artifact authentication, interpreter/source
//! closure identity, engine selection or OS network containment. The caller owns
//! request semantics, stdio/environment/cwd/executable pins and supervision.
//!
//! Inputs are caller-trusted bytes up to 65536. This module seals them in private
//! memfds, stages copies above the reserved targets, marks inherited fds >=5
//! close-on-exec without closing Rust's exec-error channel, and remaps exact fd3
//! (config) / fd4 (expected). Original and reservation descriptors live with the
//! Command so fd3/fd4 cannot be reused by Command's private spawn machinery. Install
//! once, with no later pre_exec callback that changes descriptor policy. Tests use
//! the current test executable as a child helper; no engine or daemon is involved.
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

    /// Read-only original config descriptor plus sealed expected bytes. Caller
    /// owns held-file validation; parent descriptor flags are left untouched.
    pub(super) fn with_held_config(config: &File, expected: &[u8]) -> io::Result<Self> {
        if expected.len() > MAX_INPUT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "runtime expected too large",
            ));
        }
        let fd = unsafe { libc::fcntl(config.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 5) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            config: unsafe { OwnedFd::from_raw_fd(fd) },
            expected: sealed(expected)?,
        })
    }
    /// Consume inputs into the Command so their lifetime covers every spawn.
    /// Callers own stdio, environment, cwd, executable pins and supervision.
    /// Install once, with no subsequent pre_exec callback that changes fd policy.
    pub(super) fn install(self, command: &mut Command) -> io::Result<()> {
        self.install_with(command, close_range_inherited)
    }

    // The close-range operation is a parameter so a test can exercise the
    // ENOSYS fail-closed path without a process-global flag or cfg(test) code
    // inside the production runner. Production install always passes the real
    // syscall wrapper; semantics are unchanged.
    fn install_with(
        self,
        command: &mut Command,
        mark_inherited_close_on_exec: fn() -> io::Result<()>,
    ) -> io::Result<()> {
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
                mark_inherited_close_on_exec()?;
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

fn close_range_inherited() -> io::Result<()> {
    if unsafe {
        libc::syscall(
            libc::SYS_close_range,
            5u32,
            u32::MAX,
            libc::CLOSE_RANGE_CLOEXEC,
        )
    } == -1
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
fn close_range_enosys() -> io::Result<()> {
    Err(io::Error::from_raw_os_error(libc::ENOSYS))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::fs::{self, File};
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use tempfile::TempDir;

    const HELPER_ENV: &str = "HG_FD_INPUTS_HELPER";
    const HELPER_OUT: &str = "HG_FD_INPUTS_OUT";
    const HELPER_TEST: &str = "process::fd_inputs::tests::helper";
    const CONFIG: &[u8] = b"CONFIG-3";
    const EXPECTED: &[u8] = b"EXPECTED-4";
    const TARGET_FDS: [RawFd; 5] = [0, 1, 2, 3, 4];

    fn hex(bytes: &[u8]) -> String {
        let mut value = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut value, "{byte:02x}").expect("hex");
        }
        value
    }

    fn from_hex(value: &str) -> Vec<u8> {
        assert_eq!(value.len() % 2, 0);
        (0..value.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&value[index..index + 2], 16).expect("hex"))
            .collect()
    }

    fn read_fd(fd: RawFd, limit: usize) -> io::Result<Vec<u8>> {
        let mut output = Vec::new();
        let mut buffer = [0u8; 4096];
        let mut offset = 0u64;
        loop {
            let count = unsafe {
                libc::pread(
                    fd,
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    offset as libc::off_t,
                )
            };
            if count < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if count == 0 {
                return Ok(output);
            }
            if output.len() + count as usize > limit {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "too large"));
            }
            output.extend_from_slice(&buffer[..count as usize]);
            offset += count as u64;
        }
    }

    fn live_fds() -> io::Result<Vec<RawFd>> {
        let names: Vec<RawFd> = {
            let entries = fs::read_dir("/proc/self/fd")?;
            entries
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| entry.file_name().to_string_lossy().parse().ok())
                .collect()
        };
        let mut fds = Vec::new();
        for fd in names {
            let mut status: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(fd, &mut status) } == 0 {
                fds.push(fd);
            }
        }
        fds.sort_unstable();
        Ok(fds)
    }

    fn fd_stat(fd: RawFd) -> libc::stat {
        let mut status: libc::stat = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::fstat(fd, &mut status) }, 0);
        status
    }

    fn close_fd(fd: RawFd) {
        unsafe {
            libc::close(fd);
        }
    }

    fn clear_cloexec(fd: RawFd) {
        assert_ne!(unsafe { libc::fcntl(fd, libc::F_SETFD, 0) }, -1);
    }

    fn high_inherited_fd() -> OwnedFd {
        let file = File::open("/dev/null").expect("null");
        let fd = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 100) };
        assert!(fd >= 100);
        clear_cloexec(fd);
        unsafe { OwnedFd::from_raw_fd(fd) }
    }

    fn helper_command(mode: &str, out: &Path) -> Command {
        let mut command = Command::new(std::env::current_exe().expect("test executable"));
        command.args([HELPER_TEST, "--exact", "--nocapture", "--test-threads=1"]);
        command.env(HELPER_ENV, mode);
        command.env(HELPER_OUT, out);
        command.stdin(Stdio::null());
        command
    }

    fn run_helper(mode: &str, directory: &Path) -> Value {
        let out = directory.join(format!("{mode}.json"));
        let output = helper_command(mode, &out).output().expect("spawn helper");
        assert!(
            output.status.success(),
            "helper {mode} failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = fs::read(&out).unwrap_or_else(|error| {
            panic!(
                "helper {mode} produced no result at {}: {error}",
                out.display()
            )
        });
        serde_json::from_slice(&bytes).expect("helper JSON")
    }

    fn helper_inspect(out: &Path) -> Result<(), String> {
        let fds = live_fds().map_err(|error| error.to_string())?;
        let config = read_fd(3, MAX_INPUT_BYTES + 1).map_err(|error| error.to_string())?;
        let expected = read_fd(4, MAX_INPUT_BYTES + 1).map_err(|error| error.to_string())?;
        let value = json!({
            "fds": fds,
            "config": hex(&config),
            "expected": hex(&expected),
        });
        fs::write(out, serde_json::to_vec(&value).unwrap()).map_err(|error| error.to_string())
    }

    fn target_inputs(swapped: bool) -> Result<SealedInputs, String> {
        close_fd(3);
        close_fd(4);
        let config = sealed(CONFIG).map_err(|error| error.to_string())?;
        let expected = sealed(EXPECTED).map_err(|error| error.to_string())?;
        if swapped {
            Ok(SealedInputs {
                config: expected,
                expected: config,
            })
        } else {
            Ok(SealedInputs { config, expected })
        }
    }

    fn hole_inputs() -> Result<SealedInputs, String> {
        // This helper process is disposable. Close the target numbers, then
        // occupy them with owned duplicates of stdin. SealedInputs::new must
        // therefore allocate above 4; dropping the holdings reopens the holes
        // before install without double-closing a File-owned descriptor.
        close_fd(3);
        close_fd(4);
        let mut holdings = Vec::new();
        for target in [3, 4] {
            let fd = unsafe { libc::dup(0) };
            if fd < 0 {
                return Err(io::Error::last_os_error().to_string());
            }
            if fd != target {
                close_fd(fd);
                return Err(format!("expected fd {target}, got {fd}"));
            }
            holdings.push(unsafe { OwnedFd::from_raw_fd(fd) });
        }
        let inputs = SealedInputs::new(CONFIG, EXPECTED).map_err(|error| error.to_string())?;
        drop(holdings);
        Ok(inputs)
    }

    fn helper_child(mode: &str, out: &Path, swapped: bool) -> Result<(), String> {
        let inputs = if mode == "holes" {
            hole_inputs()?
        } else {
            target_inputs(swapped)?
        };
        let child_out = out.with_extension("child.json");
        let mut command = helper_command("inspect", &child_out);
        inputs
            .install(&mut command)
            .map_err(|error| error.to_string())?;
        let output = command.output().map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "child failed: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let bytes = fs::read(&child_out).map_err(|error| error.to_string())?;
        fs::write(out, bytes).map_err(|error| error.to_string())
    }

    fn helper_held(mode: &str, out: &Path) -> Result<(), String> {
        close_fd(3);
        close_fd(4);
        let path = out.with_extension("config");
        fs::write(&path, CONFIG).map_err(|e| e.to_string())?;
        let mut original = File::open(&path).map_err(|e| e.to_string())?;
        if mode == "held_swapped" {
            let fd = unsafe { libc::fcntl(original.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 4) };
            if fd < 0 {
                return Err("dup original".into());
            }
            original = unsafe { File::from_raw_fd(fd) };
        }
        let mut inputs =
            SealedInputs::with_held_config(&original, EXPECTED).map_err(|e| e.to_string())?;
        if mode == "held_holes" {
            inputs = SealedInputs {
                config: duplicate(&inputs.config, 10).map_err(|e| e.to_string())?,
                expected: duplicate(&inputs.expected, 10).map_err(|e| e.to_string())?,
            };
        }
        drop(original);
        let child_out = out.with_extension("child.json");
        let mut command = helper_command("inspect", &child_out);
        inputs.install(&mut command).map_err(|e| e.to_string())?;
        let output = command.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("held helper failed".into());
        }
        fs::write(out, fs::read(child_out).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }

    fn helper_holes_fail(out: &Path) -> Result<(), String> {
        let inputs = hole_inputs()?;
        let mut command = Command::new("/definitely/not/a/harness-gate-executable");
        inputs
            .install(&mut command)
            .map_err(|error| error.to_string())?;
        let error = command
            .output()
            .expect_err("missing executable must fail")
            .raw_os_error();
        fs::write(
            out,
            serde_json::to_vec(&json!({"raw_os_error": error})).unwrap(),
        )
        .map_err(|error| error.to_string())
    }

    fn helper_leak(out: &Path) -> Result<(), String> {
        let before = live_fds().map_err(|error| error.to_string())?;
        let inputs = SealedInputs::new(CONFIG, EXPECTED).map_err(|error| error.to_string())?;
        let mut command = helper_command("inspect", &out.with_extension("never"));
        inputs
            .install(&mut command)
            .map_err(|error| error.to_string())?;
        drop(command);
        let after = live_fds().map_err(|error| error.to_string())?;
        fs::write(
            out,
            serde_json::to_vec(&json!({"before": before, "after": after})).unwrap(),
        )
        .map_err(|error| error.to_string())
    }

    #[test]
    fn helper() {
        let Ok(mode) = std::env::var(HELPER_ENV) else {
            return;
        };
        let out = PathBuf::from(std::env::var_os(HELPER_OUT).expect("helper output"));
        let result = match mode.as_str() {
            "inspect" => helper_inspect(&out),
            "held_targets" | "held_holes" | "held_swapped" => helper_held(&mode, &out),
            "targets" => helper_child("targets", &out, false),
            "swapped" => helper_child("swapped", &out, true),
            "holes" => helper_child("holes", &out, false),
            "holes_fail" => helper_holes_fail(&out),
            "leak" => helper_leak(&out),
            other => Err(format!("unknown helper mode {other}")),
        };
        if let Err(error) = result {
            let _ = fs::write(&out, format!("helper error: {error}"));
            std::process::exit(2);
        }
        std::process::exit(0);
    }

    #[test]
    fn sealed_inputs_are_private_bounded_and_pread_exact() {
        let inputs = SealedInputs::new(CONFIG, EXPECTED).expect("sealed inputs");
        for (fd, expected) in [
            (inputs.config.as_raw_fd(), CONFIG),
            (inputs.expected.as_raw_fd(), EXPECTED),
        ] {
            let status = fd_stat(fd);
            assert_eq!(status.st_mode & libc::S_IFMT, libc::S_IFREG);
            assert_eq!(status.st_mode & 0o777, 0o600);
            assert_eq!(status.st_uid, unsafe { libc::geteuid() });
            assert_eq!(status.st_nlink, 0);
            assert_eq!(status.st_size as usize, expected.len());
            let seals = unsafe { libc::fcntl(fd, libc::F_GET_SEALS) };
            assert_ne!(seals, -1);
            assert_eq!(seals & SEALS, SEALS);
            assert_eq!(read_fd(fd, MAX_INPUT_BYTES + 1).unwrap(), expected);
            assert_eq!(read_fd(fd, MAX_INPUT_BYTES + 1).unwrap(), expected);
            unsafe {
                libc::lseek(fd, 0, libc::SEEK_END);
            }
            assert_eq!(read_fd(fd, MAX_INPUT_BYTES + 1).unwrap(), expected);
        }
        assert!(SealedInputs::new(&vec![0; MAX_INPUT_BYTES + 1], EXPECTED).is_err());
        assert!(SealedInputs::new(CONFIG, &vec![0; MAX_INPUT_BYTES + 1]).is_err());
        let _exact = SealedInputs::new(&vec![0; MAX_INPUT_BYTES], &[]).expect("exact max");
    }

    #[test]
    fn install_preserves_bytes_and_closes_inherited_high_fd() {
        let directory = TempDir::new().unwrap();
        let high = high_inherited_fd();
        let inputs = SealedInputs::new(CONFIG, EXPECTED).expect("inputs");
        let out = directory.path().join("inspect.json");
        let mut command = helper_command("inspect", &out);
        inputs.install(&mut command).expect("install");
        let output = command.output().expect("spawn helper");
        assert!(
            output.status.success(),
            "helper failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        drop(high);
        let value: Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
        assert_eq!(value["fds"], json!(TARGET_FDS));
        assert_eq!(from_hex(value["config"].as_str().unwrap()), CONFIG);
        assert_eq!(from_hex(value["expected"].as_str().unwrap()), EXPECTED);
    }

    #[test]
    fn install_handles_target_equal_holes_and_swapped_sources() {
        let directory = TempDir::new().unwrap();
        let targets = run_helper("targets", directory.path());
        assert_eq!(targets["fds"], json!(TARGET_FDS));
        assert_eq!(from_hex(targets["config"].as_str().unwrap()), CONFIG);
        assert_eq!(from_hex(targets["expected"].as_str().unwrap()), EXPECTED);

        let swapped = run_helper("swapped", directory.path());
        assert_eq!(swapped["fds"], json!(TARGET_FDS));
        assert_eq!(from_hex(swapped["config"].as_str().unwrap()), EXPECTED);
        assert_eq!(from_hex(swapped["expected"].as_str().unwrap()), CONFIG);

        let holes = run_helper("holes", directory.path());
        assert_eq!(holes["fds"], json!(TARGET_FDS));
        assert_eq!(from_hex(holes["config"].as_str().unwrap()), CONFIG);
        assert_eq!(from_hex(holes["expected"].as_str().unwrap()), EXPECTED);
    }

    #[test]
    fn install_reports_spawn_failures_and_keeps_error_channel() {
        let directory = TempDir::new().unwrap();
        let missing = directory.path().join("missing");
        let denied = directory.path().join("denied");
        fs::write(&denied, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o644)).unwrap();
        for (path, expected) in [(missing, libc::ENOENT), (denied, libc::EACCES)] {
            let inputs = SealedInputs::new(CONFIG, EXPECTED).expect("inputs");
            let mut command = Command::new(path);
            inputs.install(&mut command).expect("install");
            let error = command.output().expect_err("spawn must fail");
            assert_eq!(error.raw_os_error(), Some(expected));
        }
        let failed_holes = run_helper("holes_fail", directory.path());
        assert_eq!(
            failed_holes["raw_os_error"].as_i64(),
            Some(libc::ENOENT as i64)
        );
    }

    #[test]
    fn install_reports_pre_exec_and_close_range_failures() {
        let inputs = SealedInputs::new(CONFIG, EXPECTED).expect("inputs");
        let mut command = Command::new("/bin/true");
        inputs.install(&mut command).expect("install");
        unsafe {
            command.pre_exec(|| Err(io::Error::from_raw_os_error(libc::EIO)));
        }
        let error = command.output().expect_err("pre_exec must fail");
        assert_eq!(error.raw_os_error(), Some(libc::EIO));

        let inputs = SealedInputs::new(CONFIG, EXPECTED).expect("inputs");
        let mut command = Command::new("/bin/true");
        inputs
            .install_with(&mut command, close_range_enosys)
            .expect("install");
        let error = command
            .output()
            .expect_err("forced close_range failure must fail closed");
        assert_eq!(error.raw_os_error(), Some(libc::ENOSYS));
    }

    #[test]
    fn command_can_be_spawned_twice_with_installed_inputs() {
        let directory = TempDir::new().unwrap();
        let out = directory.path().join("reuse.json");
        let inputs = SealedInputs::new(CONFIG, EXPECTED).expect("inputs");
        let mut command = helper_command("inspect", &out);
        inputs.install(&mut command).expect("install");
        for _ in 0..2 {
            let output = command.output().expect("spawn");
            assert!(
                output.status.success(),
                "helper failed: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let value: Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
            assert_eq!(value["fds"], json!(TARGET_FDS));
            assert_eq!(from_hex(value["config"].as_str().unwrap()), CONFIG);
            assert_eq!(from_hex(value["expected"].as_str().unwrap()), EXPECTED);
        }
    }

    #[test]
    fn parent_descriptors_release_when_command_drops() {
        let directory = TempDir::new().unwrap();
        let value = run_helper("leak", directory.path());
        assert_eq!(value["before"], value["after"]);
    }
    #[test]
    fn held_config_preserves_original_and_child_mapping() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("config");
        fs::write(&path, CONFIG).unwrap();
        let original = File::open(&path).unwrap();
        let flags = unsafe { libc::fcntl(original.as_raw_fd(), libc::F_GETFD) };
        let inputs = SealedInputs::with_held_config(&original, EXPECTED).unwrap();
        assert_eq!(
            fd_stat(inputs.config.as_raw_fd()).st_ino,
            fd_stat(original.as_raw_fd()).st_ino
        );
        let staged = inputs.config.as_raw_fd();
        let sealed = inputs.expected.as_raw_fd();
        assert_eq!(read_fd(staged, MAX_INPUT_BYTES).unwrap(), CONFIG);
        assert_eq!(
            unsafe { libc::fcntl(sealed, libc::F_GET_SEALS) } & SEALS,
            SEALS
        );
        let out = directory.path().join("inspect.json");
        let mut command = helper_command("inspect", &out);
        inputs.install(&mut command).unwrap();
        let output = command.output().unwrap();
        assert!(output.status.success());
        let value: Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
        assert_eq!(value["fds"], json!(TARGET_FDS));
        assert_eq!(from_hex(value["config"].as_str().unwrap()), CONFIG);
        assert_eq!(from_hex(value["expected"].as_str().unwrap()), EXPECTED);
        assert_eq!(
            unsafe { libc::fcntl(original.as_raw_fd(), libc::F_GETFD) },
            flags
        );
        drop(command);
        assert_eq!(unsafe { libc::fcntl(staged, libc::F_GETFD) }, -1);
        assert_eq!(unsafe { libc::fcntl(sealed, libc::F_GETFD) }, -1);
        assert_eq!(
            read_fd(original.as_raw_fd(), MAX_INPUT_BYTES).unwrap(),
            CONFIG
        );
        assert!(SealedInputs::with_held_config(&original, &vec![0; MAX_INPUT_BYTES + 1]).is_err());
    }
    #[test]
    fn held_config_target_holes_and_swapped_sources() {
        let directory = TempDir::new().unwrap();
        for mode in ["held_targets", "held_holes", "held_swapped"] {
            let value = run_helper(mode, directory.path());
            assert_eq!(value["fds"], json!(TARGET_FDS));
            assert_eq!(from_hex(value["config"].as_str().unwrap()), CONFIG);
            assert_eq!(from_hex(value["expected"].as_str().unwrap()), EXPECTED);
        }
    }
}
