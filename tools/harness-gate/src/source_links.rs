//! Opt-in navigation to locally verified committed source; never a gate decision.
use crate::{process, report_evidence};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

const BUDGET: Duration = Duration::from_secs(2);
const MAX_FILES: usize = 128;
const MAX_BYTES: usize = 1024 * 1024;

/// A deliberately narrow URL grammar: no credentials, query, fragment or templates.
pub(crate) fn valid_repository(value: &str) -> bool {
    let Some(path) = value.strip_prefix("https://github.com/") else {
        return false;
    };
    let parts: Vec<_> = path.split('/').collect();
    parts.len() == 2 && parts.iter().all(|part| repository_part(part)) && !path.ends_with(".git")
}
fn repository_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
fn object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn source_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && !value.chars().any(|c| c.is_control() || "\\:%".contains(c))
        && value
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".."))
}
fn encode_path(value: &str) -> String {
    use std::fmt::Write;
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            write!(&mut encoded, "%{byte:02X}").expect("write to String");
        }
    }
    encoded
}
struct Blob {
    digest: String,
    lines: u64,
}
pub(crate) struct Links<'a> {
    repository: &'a str,
    commit: &'a str,
    root: &'a Path,
    started: Instant,
    blobs: BTreeMap<String, Option<Blob>>,
}
impl<'a> Links<'a> {
    pub(crate) fn new(
        repository: Option<&'a str>,
        commit: &'a str,
        root: &'a Path,
    ) -> Option<Self> {
        let repository = repository.filter(|value| valid_repository(value))?;
        object_id(commit).then(|| Self {
            repository,
            commit,
            root,
            started: Instant::now(),
            blobs: BTreeMap::new(),
        })
    }
    pub(crate) fn link(
        &mut self,
        gate: &Value,
        index: &report_evidence::Index<'_>,
    ) -> Option<String> {
        let record = &gate["record"];
        let evidence = index
            .resolve_agreement(gate, record["metric"].as_str()?)
            .ok()?;
        if evidence["context"]["commit"] != self.commit {
            return None;
        }
        let subject = &evidence["subject"];
        let path = subject["path"]
            .as_str()
            .filter(|value| source_path(value))?;
        let blob = self.blob(path)?;
        let (start, end) = line_range(subject, blob)?;
        Some(format!(
            "{}/blob/{}/{}#L{start}-L{end}",
            self.repository,
            self.commit,
            encode_path(path)
        ))
    }
    fn blob(&mut self, path: &str) -> Option<&Blob> {
        if !self.blobs.contains_key(path) {
            if self.blobs.len() >= MAX_FILES {
                return None;
            }
            let blob = self.read_blob(path);
            self.blobs.insert(path.to_owned(), blob);
        }
        self.blobs.get(path)?.as_ref()
    }
    fn read_blob(&self, path: &str) -> Option<Blob> {
        let tree = self.git(
            &[
                "ls-tree",
                "-z",
                "--full-tree",
                self.commit,
                "--",
                &format!(":(literal){path}"),
            ],
            8192,
        )?;
        let id = tree_blob(&tree, path)?;
        let bytes = self.git(&["cat-file", "blob", id], MAX_BYTES)?;
        let text = std::str::from_utf8(&bytes).ok()?;
        Some(Blob {
            digest: format!("{:x}", Sha256::digest(&bytes)),
            lines: text.lines().count() as u64,
        })
    }
    fn git(&self, args: &[&str], limit: usize) -> Option<Vec<u8>> {
        let remaining = BUDGET.checked_sub(self.started.elapsed())?;
        let args: Vec<String> = [
            "--no-lazy-fetch",
            "--no-replace-objects",
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
        ]
        .into_iter()
        .chain(args.iter().copied())
        .map(str::to_owned)
        .collect();
        let output = process::capture_with_limits(
            "git",
            &args,
            self.root,
            remaining.min(Duration::from_millis(500)),
            process::CaptureLimits {
                stdout_bytes: limit,
                stderr_bytes: 4096,
                total_bytes: limit + 4096,
                reader_deadline: Duration::from_millis(100),
            },
        )
        .ok()?;
        output.status.success().then_some(output.stdout)
    }
}

fn line_range(subject: &Value, blob: &Blob) -> Option<(u64, u64)> {
    let start = subject["span"]["start_line"].as_u64()?;
    let end = subject["span"]["end_line"].as_u64()?;
    (start > 0 && end >= start && end <= blob.lines && subject["source_sha256"] == blob.digest)
        .then_some((start, end))
}

fn tree_blob<'a>(tree: &'a [u8], path: &str) -> Option<&'a str> {
    let tree = std::str::from_utf8(tree).ok()?.strip_suffix('\0')?;
    let (header, found) = tree.split_once('\t')?;
    let fields: Vec<_> = header.split(' ').collect();
    if found != path
        || fields.len() != 3
        || !matches!(fields[0], "100644" | "100755")
        || fields[1] != "blob"
        || !object_id(fields[2])
    {
        return None;
    }
    Some(fields[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destinations_and_paths_cannot_change_origin_or_revision() {
        assert!(valid_repository("https://github.com/musutrade/GateHouse"));
        for bad in [
            "",
            "http://github.com/a/b",
            "https://github.com.evil/a/b",
            "https://user:secret@github.com/a/b",
            "https://github.com/a/b?token=secret",
            "https://github.com/a/b#ref",
            "https://github.com/a/b/",
            "https://github.com/a/b.git",
            "https://github.com/../b",
            "https://github.com/a/%62",
            "javascript:alert(1)",
            "https://github.com/a/b\n",
            "https://github.com/a/b/c",
        ] {
            assert!(!valid_repository(bad), "{bad}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "../secret",
            "src/../secret",
            "src//a",
            "./a",
            "C:/a",
            "src\\a",
            "%2e%2e/a",
            "src/a\n",
            "src/./a",
        ] {
            assert!(!source_path(bad), "{bad}");
        }
        assert!(!source_path(&"a".repeat(4097)));
        assert!(!valid_repository(&format!(
            "https://github.com/a/{}",
            "a".repeat(101)
        )));
        assert!(source_path("src/a b#?é.rs"));
        assert_eq!(encode_path("src/a b#?é.rs"), "src/a%20b%23%3F%C3%A9.rs");
        assert!(object_id(&"a".repeat(40)) && object_id(&"b".repeat(64)));
        for bad in ["main", "abcdef", &"g".repeat(40)] {
            assert!(!object_id(bad));
        }
        assert!(Links::new(None, &"a".repeat(40), Path::new(".")).is_none());
        assert!(Links::new(Some("https://github.com/a/b"), "unknown", Path::new(".")).is_none());
        assert!(Links::new(Some("javascript:alert(1)"), &"a".repeat(40), Path::new(".")).is_none());
    }
    #[test]
    fn git_tree_requires_one_exact_regular_file() {
        let id = "a".repeat(40);
        for mode in ["100644", "100755"] {
            assert_eq!(
                tree_blob(format!("{mode} blob {id}\tsrc/a\0").as_bytes(), "src/a"),
                Some(id.as_str())
            );
        }
        for bad in [
            format!("120000 blob {id}\tsrc/a\0"),
            format!("160000 commit {id}\tsrc/a\0"),
            format!("100644 blob {id}\tsrc/other\0"),
            "100644 blob nope\tsrc/a\0".into(),
            format!("100644 blob {id}\tsrc/a"),
            format!("100644 blob {id}\tsrc/a\0extra\0"),
            "no header\0".into(),
        ] {
            assert!(tree_blob(bad.as_bytes(), "src/a").is_none(), "{bad:?}");
        }
        assert!(tree_blob(b"\xff\0", "src/a").is_none());
    }
    fn git(root: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    #[test]
    fn committed_blobs_are_cached_bounded_and_independent_of_worktree_edits() {
        let workspace = crate::test_support::TestWorkspace::new("source-links");
        workspace.init_git();
        std::fs::write(workspace.join("a file.rs"), b"one\ntwo\n").unwrap();
        std::fs::write(workspace.join("binary"), b"\xff").unwrap();
        std::fs::write(workspace.join("large"), vec![b'a'; MAX_BYTES + 1]).unwrap();
        git(&workspace, &["add", "."]);
        git(&workspace, &["commit", "-qm", "source fixture"]);
        let commit = git(&workspace, &["rev-parse", "HEAD"]);
        std::fs::write(workspace.join("a file.rs"), b"dirty\n").unwrap();
        let mut links = Links::new(Some("https://github.com/a/b"), &commit, &workspace).unwrap();
        let blob = links.blob("a file.rs").unwrap();
        assert_eq!(blob.digest, format!("{:x}", Sha256::digest(b"one\ntwo\n")));
        assert_eq!(blob.lines, 2);
        assert!(links.blob("binary").is_none());
        assert!(links.blob("missing").is_none());
        assert!(links.blob("large").is_none());
        links.started = Instant::now() - BUDGET;
        assert!(links.blob("a file.rs").is_some());
        assert!(links.blob("uncached").is_none());
        for i in 0..MAX_FILES {
            links.blobs.insert(format!("budget-{i}"), None);
        }
        assert!(links.blob("over-file-budget").is_none());
        assert!(!links.blobs.contains_key("over-file-budget"));
    }
}
