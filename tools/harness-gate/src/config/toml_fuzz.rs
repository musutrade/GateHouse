//! Deterministic, bounded loader mutation smoke test; not coverage-guided fuzzing.
use super::{FlowConfig, MINIMAL_CONFIG_SNIPPET};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SEEDS: [u64; 2] = [0x59b2_2026_1016, 0x9e37_79b9_7f4a_7c15];
const CASES: usize = 128;
const MAX_BYTES: usize = 4096;
const CHILD: &str = "GATEHOUSE_TOML_MUTATION_CHILD";

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn mutate(source: &str, state: &mut u64) -> String {
    const TOKENS: &[char] = &[
        '[', ']', '{', '}', '"', '\'', '=', '#', '$', ':', '-', '0', '9', '\n', '\0', '\\', '界',
        'é',
    ];
    let mut chars: Vec<char> = source.chars().collect();
    for _ in 0..1 + next(state) % 16 {
        let at = (next(state) % (chars.len() + 1) as u64) as usize;
        let token = TOKENS[(next(state) % TOKENS.len() as u64) as usize];
        match next(state) % 3 {
            0 if at < chars.len() => {
                chars.remove(at);
            }
            1 if at < chars.len() => chars[at] = token,
            _ => chars.insert(at, token),
        }
    }
    let mut output: String = chars.into_iter().collect();
    let mut end = output.len().min(MAX_BYTES);
    while !output.is_char_boundary(end) {
        end -= 1;
    }
    output.truncate(end);
    output
}

fn outcome(source: &str) -> String {
    // No repository root: validation never resolves report templates against
    // user paths. Loading only validates; it does not execute configured steps.
    match FlowConfig::from_source_with_diagnostics(source, None, None) {
        Ok(config) => format!("ok:{}", toml::to_string(&config).unwrap()),
        Err(error) => {
            let report = error.report();
            assert!(!report.valid);
            assert!(!report.diagnostics.is_empty());
            format!("err:{}", serde_json::to_string(&report).unwrap())
        }
    }
}

fn exercise() {
    let valid = MINIMAL_CONFIG_SNIPPET;
    // Fixed valid cases test the actual interpolation + validation semantics,
    // preventing a loader that rejects everything from satisfying the harness.
    for count in 0..32 {
        let name = format!("project-{count}");
        let source = valid.replace("my-project", &name);
        let config = FlowConfig::from_source_with_diagnostics(&source, None, None).unwrap();
        assert_eq!(config.project.name, name);
        assert_eq!(config.steps[0].timeout_secs, 60);
    }
    let default = valid.replace("my-project", "${TOML_FUZZ_UNSET:-fallback}");
    assert_eq!(
        FlowConfig::from_source_with_diagnostics(&default, None, None)
            .unwrap()
            .project
            .name,
        "fallback"
    );
    for source in [
        "version = [",
        "version = 2\nversion = 2",
        &valid.replace("my-project", "${TOML_FUZZ_UNSET}"),
    ] {
        assert!(outcome(source).starts_with("err:"), "{source:?}");
    }
    let corpus = [
        valid.to_owned(),
        default,
        "version = 2\n[project]\nname = '\u{754c}'".into(),
        "version = 2\n[paths]\naudit_config = '${UNSET}'".into(),
        "x = [1, { nested = [true, false] }]".into(),
        "# ${UNSET}\nx = \"\"\"multiline\ntext\"\"\"".into(),
        String::new(),
    ];
    let mut calls = 0;
    for seed in SEEDS {
        let mut state = seed;
        for case in 0..CASES {
            let input = mutate(&corpus[case % corpus.len()], &mut state);
            assert!(input.len() <= MAX_BYTES);
            std::fs::write(
                "last-case.txt",
                format!("seed={seed:#x} case={case} input={input:?}"),
            )
            .unwrap();
            let result = std::panic::catch_unwind(|| {
                let first = outcome(&input);
                assert_eq!(first, outcome(&input));
            });
            assert!(result.is_ok(), "seed={seed:#x} case={case} input={input:?}");
            calls += 2;
        }
    }
    assert_eq!(calls, SEEDS.len() * CASES * 2);
    eprintln!("TOML mutations: {calls} loader calls, seeds={SEEDS:x?}, max={MAX_BYTES} bytes");
}

#[test]
fn bounded_toml_loader_mutations() {
    if std::env::var_os(CHILD).is_some() {
        exercise();
        return;
    }
    let workspace = tempfile::tempdir().unwrap();
    let log_path = workspace.path().join("child.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "config::toml_fuzz::bounded_toml_loader_mutations",
            "--nocapture",
        ])
        .env_clear()
        .env(CHILD, "1")
        .current_dir(workspace.path())
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log);
    // Windows may require this OS loader setting; no project overrides survive.
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let last = std::fs::read_to_string(workspace.path().join("last-case.txt"))
                .unwrap_or_else(|_| "before mutation corpus".into());
            panic!("TOML mutation subprocess exceeded 30 seconds: {last}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let log = std::fs::read_to_string(log_path).unwrap();
    let last = std::fs::read_to_string(workspace.path().join("last-case.txt"))
        .unwrap_or_else(|_| "before mutation corpus".into());
    assert!(status.success(), "{last}\n{log}");
    assert!(
        log.contains("TOML mutations: 512 loader calls"),
        "child must execute corpus: {log}"
    );
}
