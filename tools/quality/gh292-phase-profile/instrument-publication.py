#!/usr/bin/env python3
"""Instrument an external report.rs copy only; preserve every existing test/assertion."""
import argparse
import difflib
import hashlib
import json
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('--source', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
a = p.parse_args()
original = a.source.read_text()
assert 'mod gh292_publication_profile' not in original
text = original
names = ['write', 'write_report_documents', 'machine_result_with_assessment', 'write_machine_result',
         'write_generated_json', 'write_report_file', 'assess_evidence']
for name in names:
    token = 'fn ' + name + '('
    assert text.count(token) == 1, name
    start = text.index(token)
    brace = text.index('{', start)
    extra = '\n    let mut _gh292_profile = gh292_publication_profile::Phase::begin("' + name + '");'
    if name == 'write_report_file':
        extra += '\n    if let Some(phase) = _gh292_profile.as_mut() { phase.output(relative, contents.as_ref().len()); }'
    elif name == 'write_generated_json':
        extra += '\n    if let Some(phase) = _gh292_profile.as_mut() { phase.output(relative, bytes.len()); }'
    text = text[:brace + 1] + extra + text[brace + 1:]
replacements = {
    'let mut value = serde_json::to_value(value).context("serialize Core JSON model")?;':
        'let mut value = gh292_publication_profile::scope("json_value_conversion", || serde_json::to_value(value).context("serialize Core JSON model"))?;',
    'redact_json(&mut value)?;':
        'gh292_publication_profile::scope("json_recursive_redaction", || redact_json(&mut value))?;',
    'let bytes = serde_json::to_vec_pretty(&value)?;':
        'let bytes = gh292_publication_profile::scope("json_pretty_serialization", || serde_json::to_vec_pretty(&value))?;',
}
for before, after in replacements.items():
    assert text.count(before) == 1, before
    text = text.replace(before, after)
text += r'''

// External Linux diagnostic only. This module is never applied to product source.
#[allow(dead_code)]
mod gh292_publication_profile {
    use std::{collections::BTreeMap, fs, io::Write, path::PathBuf, time::Instant};
    use serde_json::json;
    struct Stats { cpu: f64, rss: u64, io: BTreeMap<String, u64> }
    fn stats() -> Stats {
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) }, 0);
        let cpu = usage.ru_utime.tv_sec as f64 + usage.ru_utime.tv_usec as f64 / 1e6
            + usage.ru_stime.tv_sec as f64 + usage.ru_stime.tv_usec as f64 / 1e6;
        let status = fs::read_to_string("/proc/self/status").unwrap();
        let rss = status.lines().find_map(|line| line.strip_prefix("VmHWM:")
            .map(|value| value.split_whitespace().next().unwrap().parse().unwrap())).unwrap();
        let io = fs::read_to_string("/proc/self/io").unwrap().lines().map(|line| {
            let (key, value) = line.split_once(':').unwrap();
            (key.to_owned(), value.trim().parse().unwrap())
        }).collect();
        Stats { cpu, rss, io }
    }
    pub(super) struct Phase { name: &'static str, path: PathBuf, start: Instant, before: Stats, output: Option<(String, usize)> }
    impl Phase {
        pub(super) fn begin(name: &'static str) -> Option<Self> {
            let path = std::env::var_os("GH292_PUBLICATION_PROFILE")?;
            Some(Self { name, path: PathBuf::from(path), before: stats(), start: Instant::now(), output: None })
        }
    }
    impl Phase { pub(super) fn output(&mut self, name: &str, bytes: usize) { self.output = Some((name.to_owned(), bytes)); } }
    impl Drop for Phase {
        fn drop(&mut self) {
            let wall = self.start.elapsed().as_secs_f64();
            let after = stats();
            let io: BTreeMap<_, _> = after.io.iter().map(|(key, value)|
                (key, value - self.before.io[key])).collect();
            let row = json!({"phase": self.name, "wall_seconds": wall,
                "cpu_seconds": after.cpu - self.before.cpu, "peak_rss_kib_cumulative": after.rss,
                "kernel_io_delta": io, "pid": std::process::id(),
                "thread": format!("{:?}", std::thread::current().id()), "written_output": self.output});
            let mut output = fs::OpenOptions::new().create(true).append(true).open(&self.path).unwrap();
            writeln!(output, "{}", row).unwrap();
        }
    }
    pub(super) fn scope<T>(name: &'static str, f: impl FnOnce() -> T) -> T {
        let _phase = Phase::begin(name);
        f()
    }
}
'''
a.output.parent.mkdir(parents=True, exist_ok=True)
with a.output.open('x') as f:
    f.write(text)
patch = ''.join(difflib.unified_diff(original.splitlines(True), text.splitlines(True),
                                   fromfile=str(a.source), tofile=str(a.output)))
a.output.with_suffix('.instrumentation.patch').write_text(patch)
a.output.with_suffix('.instrumentation.json').write_text(json.dumps({
    'source_sha256': hashlib.sha256(a.source.read_bytes()).hexdigest(),
    'output_sha256': hashlib.sha256(a.output.read_bytes()).hexdigest(),
    'scope': 'External Linux publication diagnostic; original tests and assertions unchanged.',
    'phases': names + ['json_value_conversion', 'json_recursive_redaction', 'json_pretty_serialization'],
    'boundaries': ['Nested phase totals overlap; do not add them.',
                   'CPU/I/O include trace instrumentation; VmHWM is cumulative.',
                   'No collector execution, no policy/report shape or expected value changes.',
                   'Use identical instrumentation and compiler in baseline and candidate snapshots.'],
}, indent=2) + '\n')
print(a.output)
