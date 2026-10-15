use std::process::Command;

#[test]
fn discovery_is_project_independent_and_selectors_are_exclusive() {
    let root = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .current_dir(root.path())
            .args(args)
            .output()
            .unwrap()
    };
    let listed = run(&["explain", "--list", "--json"]);
    assert!(listed.status.success());
    assert!(listed.stderr.is_empty());
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(entries.len(), 71);
    assert!(entries
        .windows(2)
        .all(|pair| pair[0]["code"].as_str() < pair[1]["code"].as_str()));
    for entry in &entries {
        let code = entry["code"].as_str().unwrap();
        let output = run(&["explain", code, "--json"]);
        assert!(output.status.success(), "{code}");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            *entry
        );
    }
    let detailed: Vec<_> = entries
        .iter()
        .filter(|entry| entry.get("details").is_some())
        .collect();
    assert_eq!(detailed.len(), 42);
    for entry in detailed {
        let details = entry["details"].as_str().unwrap();
        for heading in ["## Why this fails", "## Diagnose and repair", "## Example"] {
            assert!(details.contains(heading));
        }
        let human = run(&["explain", entry["code"].as_str().unwrap()]);
        assert!(human.status.success());
        assert!(human.stderr.is_empty());
        assert!(String::from_utf8(human.stdout).unwrap().contains(details));
    }
    for query in ["coverage", "COVERAGE", "E1401", "HGCFG"] {
        let result = run(&["explain", "--search", query, "--json"]);
        assert!(result.status.success());
        let found: Vec<serde_json::Value> = serde_json::from_slice(&result.stdout).unwrap();
        let expected: Vec<_> = entries
            .iter()
            .filter(|entry| {
                format!(
                    "{} {} {}",
                    entry["code"].as_str().unwrap(),
                    entry["title"].as_str().unwrap(),
                    entry["help"].as_str().unwrap()
                )
                .to_ascii_lowercase()
                .contains(&query.to_ascii_lowercase())
            })
            .cloned()
            .collect();
        assert!(!expected.is_empty());
        assert_eq!(found, expected);
    }
    let empty = run(&["explain", "--search", "nonexistent-xyz", "--json"]);
    assert!(empty.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&empty.stdout).unwrap(),
        serde_json::json!([])
    );
    let human = run(&["explain", "--list"]);
    assert!(human.status.success());
    let expected_human: String = entries
        .iter()
        .map(|entry| {
            format!(
                "{}: {}\n",
                entry["code"].as_str().unwrap(),
                entry["title"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(String::from_utf8(human.stdout).unwrap(), expected_human);
    let human_empty = run(&["explain", "--search", "nonexistent-xyz"]);
    assert!(human_empty.status.success());
    assert!(human_empty.stderr.is_empty());
    assert_eq!(human_empty.stdout, b"No matching error codes.\n");
    let missing_config = run(&[
        "--project-root",
        "missing-project",
        "--config",
        "missing.toml",
        "explain",
        "--list",
        "--json",
    ]);
    assert!(missing_config.status.success());
    assert!(missing_config.stderr.is_empty());
    assert_eq!(missing_config.stdout, listed.stdout);
    for args in [
        vec!["explain"],
        vec!["explain", "--search", " "],
        vec!["explain", "--list", "--search", "coverage"],
        vec!["explain", "QUALITY_BLOCKED", "--list"],
        vec!["explain", "E1401", "--search", "coverage"],
    ] {
        let result = run(&args);
        assert!(!result.status.success(), "{args:?}");
        assert!(result.stdout.is_empty());
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
