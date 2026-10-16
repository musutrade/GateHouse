use std::process::Command;
#[test]
fn interactive_requires_terminal_and_rejects_explicit_preset() {
    let root = tempfile::tempdir().unwrap();
    for (args, message) in [
        (
            vec!["init", "--interactive"],
            "requires terminal input and stderr",
        ),
        (
            vec!["init", "--interactive", "--preset", "generic"],
            "cannot be used with",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(root.path())
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
        assert!(!root.path().join(".harness-gate").exists());
    }
}

#[test]
fn explicit_project_name_validates_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("not-created");
    for name in ["", "UPPER", "bad/name", " bad", "bad ", "bad\nname"] {
        let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(&target)
            .args(["init", "--project-name", name])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{name:?}");
        assert!(!target.exists());
    }
    for name in ["q", "7.valid_id-", "a"] {
        let target = root.path().join(name);
        assert!(Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(&target)
            .args(["init", "--preset", "generic", "--project-name", name])
            .output()
            .unwrap()
            .status
            .success());
        let source = std::fs::read_to_string(target.join(".harness-gate/flow.toml")).unwrap();
        let config: toml::Value = toml::from_str(&source).unwrap();
        assert_eq!(config["project"]["name"].as_str(), Some(name));
        assert!(Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(&target)
            .args(["config", "check"])
            .output()
            .unwrap()
            .status
            .success());
    }
}
#[cfg(unix)]
mod terminal {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::process::{Child, Stdio};
    use std::time::{Duration, Instant};
    struct ChildGuard(Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn terminals() -> (fs::File, fs::File) {
        let (mut master, mut slave) = (-1, -1);
        // SAFETY: openpty receives valid writable descriptor pointers; optional
        // name, terminal settings and window size pointers are null defaults.
        let result = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(result, 0, "{}", std::io::Error::last_os_error());
        // SAFETY: successful openpty returned two owned, distinct descriptors.
        let (master, slave) =
            unsafe { (fs::File::from_raw_fd(master), fs::File::from_raw_fd(slave)) };
        // SAFETY: master is a live owned descriptor and F_SETFL takes an integer
        // flag argument. Nonblocking reads keep the test deadline enforceable.
        assert_ne!(
            unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) },
            -1
        );
        (master, slave)
    }

    fn run(root: &std::path::Path, answer: &[u8], force: bool) -> bool {
        run_named(root, answer, force, None)
    }

    fn run_named(root: &std::path::Path, answer: &[u8], force: bool, name: Option<&str>) -> bool {
        let (mut master, slave) = terminals();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
        cmd.arg("--project-root")
            .arg(root)
            .args(["init", "--interactive"]);
        if let Some(name) = name {
            cmd.args(["--project-name", name]);
        }
        if force {
            cmd.arg("--force");
        }
        let mut child = ChildGuard(
            cmd.stdin(slave.try_clone().unwrap())
                .stderr(slave)
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut output = Vec::new();
        let mut sent = false;
        loop {
            assert!(
                Instant::now() < deadline,
                "{}",
                String::from_utf8_lossy(&output)
            );
            let mut buffer = [0; 4096];
            match master.read(&mut buffer) {
                Ok(n) => output.extend_from_slice(&buffer[..n]),
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.raw_os_error() == Some(libc::EIO) => {}
                Err(e) => panic!("{e}"),
            }
            if !sent && String::from_utf8_lossy(&output).contains("Preset number") {
                master.write_all(answer).unwrap();
                sent = true;
            }
            if let Some(status) = child.0.try_wait().unwrap() {
                return status.success();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn terminal_init_confirmation_cancel_and_overwrite_boundary() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("not-created");
        assert!(!run(&missing, b"q\n", false));
        assert!(!missing.exists());
        for answer in [
            b"q\n".as_slice(),
            b"1\n\nn\n",
            b"\x04",
            b"1\n\x04",
            b"2\ny\n",
        ] {
            assert!(!run(root.path(), answer, false));
            assert!(!root.path().join(".harness-gate").exists());
        }
        assert!(!run(root.path(), b"1\nUPPER\ny\n", false));
        assert!(!root.path().join(".harness-gate").exists());
        assert!(run(root.path(), b"1\nchosen-name\ny\n", false));
        let checked = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(root.path())
            .args(["config", "check"])
            .output()
            .unwrap();
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
        let path = root.path().join(".harness-gate/flow.toml");
        let source = fs::read_to_string(&path).unwrap();
        let value: toml::Value = toml::from_str(&source).unwrap();
        assert_eq!(value["version"].as_integer(), Some(2));
        assert_eq!(value["project"]["name"].as_str(), Some("chosen-name"));
        fs::write(&path, format!("{source}\n# preserve\n")).unwrap();
        assert!(!run(root.path(), b"1\n\ny\n", false));
        assert!(fs::read_to_string(&path).unwrap().contains("# preserve"));
        assert!(!run(root.path(), b"1\n\nn\n", true));
        assert!(fs::read_to_string(&path).unwrap().contains("# preserve"));
        assert!(run(root.path(), b"1\n\ny\n", true));
        assert!(!fs::read_to_string(&path).unwrap().contains("# preserve"));
    }
    #[test]
    fn explicit_name_is_an_editable_interactive_default() {
        for (answer, expected) in [
            (b"1\n\ny\n".as_slice(), "flag-default"),
            (b"1\nq\ny\n", "q"),
        ] {
            let root = tempfile::tempdir().unwrap();
            assert!(run_named(root.path(), answer, false, Some("flag-default")));
            let source = fs::read_to_string(root.path().join(".harness-gate/flow.toml")).unwrap();
            let config: toml::Value = toml::from_str(&source).unwrap();
            assert_eq!(config["project"]["name"].as_str(), Some(expected));
        }
    }
}
