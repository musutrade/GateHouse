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
        let target = root.path().join(format!("project space {name}"));
        assert!(Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(&target)
            .args(["init", "--preset", "generic", "--project-name", name])
            .output()
            .unwrap()
            .status
            .success());
        let repeated = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(&target)
            .args(["init", "--force", "--project-name", name])
            .output()
            .unwrap();
        assert!(repeated.status.success());
        let guidance = String::from_utf8(repeated.stdout).unwrap();
        assert!(guidance
            .lines()
            .any(|line| line == "  harness-gate config check"));
        assert!(!guidance.contains("--project-root"));
        assert!(guidance.contains(
            &target
                .canonicalize()
                .unwrap()
                .display()
                .to_string()
                .escape_debug()
                .to_string()
        ));
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
        run_catalog(root, answer, force, name, None)
    }

    fn run_catalog(
        root: &std::path::Path,
        answer: &[u8],
        force: bool,
        name: Option<&str>,
        catalog: Option<&std::path::Path>,
    ) -> bool {
        let (mut master, slave) = terminals();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_harness-gate"));
        cmd.arg("--project-root")
            .arg(root)
            .args(["init", "--interactive"]);
        if let Some(name) = name {
            cmd.args(["--project-name", name]);
        }
        if let Some(catalog) = catalog {
            cmd.arg("--presets-dir").arg(catalog);
        }
        if force {
            cmd.arg("--force");
        }
        let mut child = ChildGuard(
            cmd.stdin(slave.try_clone().unwrap())
                .stderr(slave)
                .stdout(Stdio::piped())
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
                if !status.success() {
                    let mut stdout = String::new();
                    child
                        .0
                        .stdout
                        .take()
                        .unwrap()
                        .read_to_string(&mut stdout)
                        .unwrap();
                    assert!(stdout.is_empty(), "unexpected success guidance: {stdout}");
                }
                return status.success();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn terminal_init_confirmation_cancel_and_overwrite_boundary() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("not-created");
        for answer in [b"q\n".as_slice(), b"0\nq\n", b"0\n\x04", b"1\nUPPER\n\x04"] {
            assert!(!run(&missing, answer, false));
            assert!(!missing.exists());
        }
        for answer in [
            b"q\n".as_slice(),
            b"1\n\nn\n",
            b"\x04",
            b"1\n\x04",
            b"2\ny\n\x04",
        ] {
            assert!(!run(root.path(), answer, false));
            assert!(!root.path().join(".harness-gate").exists());
        }
        assert!(!run(root.path(), b"1\nUPPER\n\x04", false));
        assert!(!root.path().join(".harness-gate").exists());
        assert!(run(
            root.path(),
            b"0\nunknown\nd\n1\nUPPER\nchosen-name\ny\n",
            false
        ));
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
    #[test]
    fn catalog_suggestion_is_explicit_overridable_and_cancellable() {
        let catalog = tempfile::tempdir().unwrap();
        fs::write(
            catalog.path().join("flow.toml"),
            include_str!("../presets/generic.flow.toml")
                .replace("Git whitespace check", "Catalog-selected checks"),
        )
        .unwrap();
        fs::write(catalog.path().join("catalog.json"), r#"{"schema":"harness-preset-catalog/v2","presets":{"custom":{"description":"Owner-reviewed checks","flow":"flow.toml","detect_files":["Cargo.toml"]}},"packs":{}}"#).unwrap();
        for (answer, expected) in [
            (b"d\n\ny\n".as_slice(), Some("Catalog-selected checks")),
            (b"1\n\ny\n", Some("Git whitespace check")),
            (b"\n", None),
            (b"q\n", None),
            (b"\x04", None),
        ] {
            let project = tempfile::tempdir().unwrap();
            fs::write(
                project.path().join("Cargo.toml"),
                "contents are not evaluated",
            )
            .unwrap();
            assert_eq!(
                run_catalog(project.path(), answer, false, None, Some(catalog.path())),
                expected.is_some()
            );
            let path = project.path().join(".harness-gate/flow.toml");
            if let Some(expected) = expected {
                let config: toml::Value =
                    toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
                assert_eq!(config["steps"][0]["label"].as_str(), Some(expected));
                assert!(Command::new(env!("CARGO_BIN_EXE_harness-gate"))
                    .arg("--project-root")
                    .arg(project.path())
                    .args(["config", "check"])
                    .output()
                    .unwrap()
                    .status
                    .success());
            } else {
                assert!(!path.exists());
            }
        }
        let project = tempfile::tempdir().unwrap();
        assert!(!run_catalog(
            project.path(),
            b"d\n\x04",
            false,
            None,
            Some(catalog.path())
        ));
        assert!(!project.path().join(".harness-gate").exists());
    }
}

#[cfg(unix)]
#[test]
fn initialization_guidance_escapes_paths_and_does_not_execute_declared_commands() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace
        .path()
        .join("project space;$(touch injected)\n\u{1b}[2J");
    let catalog = workspace.path().join("catalog");
    std::fs::create_dir(&catalog).unwrap();
    let flow = include_str!("../presets/generic.flow.toml")
        .replace("program = \"git\"", "program = \"touch\"")
        .replace(
            "args = [\"diff\", \"--check\"]",
            "args = [\"executed-marker\"]",
        )
        .replace("args = [\"--version\"]", "args = [\"executed-marker\"]");
    std::fs::write(catalog.join("flow.toml"), flow).unwrap();
    std::fs::write(catalog.join("catalog.json"), r#"{"schema":"harness-preset-catalog/v1","presets":{"custom":{"description":"Explicit test commands","flow":"flow.toml"}},"packs":{}}"#).unwrap();
    let init = || {
        Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .arg("--project-root")
            .arg(&root)
            .args(["init", "--preset", "custom", "--presets-dir"])
            .arg(&catalog)
            .output()
            .unwrap()
    };
    let output = init();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains('\u{1b}'));
    assert!(stdout.contains("\\n\\u{1b}[2J"));
    assert!(!stdout.contains("--project-root"));
    for command in [
        "  harness-gate config check",
        "  harness-gate doctor",
        "  harness-gate verify --plan --all",
    ] {
        assert!(stdout.lines().any(|line| line == command));
    }
    assert!(stdout.contains("Execute the declared dependency checks"));
    assert!(!root.join("executed-marker").exists());
    assert!(!workspace.path().join("injected").exists());
    assert!(Command::new("git")
        .args(["init", "--quiet"])
        .arg(&root)
        .output()
        .unwrap()
        .status
        .success());
    for args in [vec!["config", "check"], vec!["verify", "--plan", "--all"]] {
        let result = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .current_dir(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert!(!root.join("executed-marker").exists());
    assert!(!root.join(".harness-gate/reports").exists());
    let denied = init();
    assert!(!denied.status.success());
    assert!(denied.stdout.is_empty());
}
