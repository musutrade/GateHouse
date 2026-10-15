#!/usr/bin/env python3
"""Optional synthetic cache timings; never invoked by required CI or adopted as a baseline."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import tempfile
import time

PROGRAM = r'''
use std::{fs, hint::black_box};
fn main() {
    let source = fs::read("source.txt").unwrap();
    let mut value = source.iter().fold(0_u64, |a, b| a.wrapping_mul(31).wrapping_add(*b as u64));
    for _ in 0..100_000_000 {
        value = black_box(value).wrapping_mul(6364136223846793005).wrapping_add(1);
    }
    println!("deterministic-result={value}");
}
'''

def run(command, cwd):
    completed = subprocess.run(command, cwd=cwd, text=True, capture_output=True)
    if completed.returncode:
        raise RuntimeError(f"{command[0]} exited {completed.returncode}: {completed.stderr}")
    return completed.stdout

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--repeats', type=int, default=7)
    parser.add_argument('--iterations', type=int, default=100_000_000)
    args = parser.parse_args()
    if platform.system() != 'Linux' or not 3 <= args.repeats <= 30 or not 1 <= args.iterations <= 2_000_000_000:
        parser.error('fixture supports Linux, 3..30 repeats and 1..2000000000 iterations')
    binary = args.binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='gatehouse-step-cache-benchmark-') as temporary:
        root = Path(temporary).resolve()
        project = root / 'project'
        project.mkdir()
        cache = root / 'host-cache'
        run([str(binary), '--project-root', str(project), 'init', '--preset', 'generic'], project)
        (project / 'source.txt').write_text('fixed synthetic cache workload/v1\n')
        program_source = PROGRAM if args.iterations == 100_000_000 else PROGRAM.replace('100_000_000', str(args.iterations))
        (project / 'checker.rs').write_text(program_source)
        program = project / 'checker'
        run(['rustc', '-O', str(project / 'checker.rs'), '-o', str(program)], project)
        dynamic = run(['ldd', str(program)], project)
        libraries = sorted({str(Path(p).resolve(strict=True)) for p in re.findall(r'(/[^\s()]+)', dynamic)})
        flow = project / '.harness-gate/flow.toml'
        header = flow.read_text().split('[[steps]]', 1)[0]
        header = header.replace('hook_profile = "hook"', 'hook_profile = "full"')
        header = re.sub(r'required_steps\s*=\s*\[[^]]*\]', 'required_steps = ["project.synthetic"]', header)
        flow.write_text(header + f'''
[[steps]]
id = "project.synthetic"
label = "Synthetic deterministic CPU check"
component = "project"
profiles = ["full"]
program = "checker"
args = []
cwd = "{{root}}"
log = "synthetic.log"
timeout_secs = 60
[steps.cache]
version = 1
deterministic = true
side_effect_free = true
inputs = []
outputs = []
toolchain = {json.dumps([str(program), *libraries])}
environment = []
''')
        (project / '.gitignore').write_text('.harness-gate/reports/\n')
        for command in [['git', 'init', '-q'], ['git', 'config', 'user.name', 'Cache fixture'],
                        ['git', 'config', 'user.email', 'fixture@example.invalid'],
                        ['git', 'add', '.'], ['git', 'commit', '-qm', 'fixed fixture']]:
            run(command, project)
        base = [str(binary), '--project-root', str(project)]
        cached = base + ['--step-cache-dir', str(cache), 'verify', '--profile', 'full', '--all']
        uncached = base + ['verify', '--profile', 'full', '--all']
        report_path = project / '.harness-gate/reports/test_result.json'
        def sample(command):
            started = time.perf_counter()
            run(command, project)
            elapsed = (time.perf_counter() - started) * 1000
            report = json.loads(report_path.read_text())
            assert report['passed'] and report['evidence_complete']
            step = next(s for s in report['steps'] if s['step_id'] == 'project.synthetic')
            return {'wall_ms': round(elapsed, 3), 'command_or_hit_ms': step['duration_ms'],
                    'cache': step['cache']}, Path(step['log']).read_bytes()
        cold, expected = sample(cached)
        assert cold['cache']['status'] == 'MISS' and cold['cache']['reason'] != 'CACHE_STORE_REJECTED'
        samples = []
        for _ in range(args.repeats):
            plain, log = sample(uncached)
            assert plain['cache']['reason'] == 'CACHE_HOST_DISABLED' and log == expected
            hit, log = sample(cached)
            assert hit['cache']['status'] == 'HIT' and log == expected
            samples.append({'uncached': plain, 'hit': hit})
        plain_median = statistics.median(s['uncached']['wall_ms'] for s in samples)
        hit_median = statistics.median(s['hit']['wall_ms'] for s in samples)
        output = {'schema': 'step-cache-synthetic-benchmark/v1',
                  'series': f'linux-deterministic-cpu-{args.iterations // 1_000_000}m/v1', 'iterations': args.iterations,
                  'adopted_baseline': False,
                  'host': {'platform': platform.platform(), 'cpu_count': os.cpu_count()},
                  'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                  'fixture_sha256': hashlib.sha256(program_source.encode()).hexdigest(),
                  'toolchain': [{'path': p, 'sha256': hashlib.sha256(Path(p).read_bytes()).hexdigest()}
                                for p in [str(program), *libraries]],
                  'core_version': run([str(binary), '--version'], project).strip(),
                  'rustc': run(['rustc', '-vV'], project).strip(),
                  'cold': cold, 'samples': samples,
                  'median': {'uncached_wall_ms': plain_median, 'hit_wall_ms': hit_median,
                             'reduction_percent': round((plain_median-hit_median)/plain_median*100, 2)}}
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(output, indent=2)+'\n')
        print(json.dumps(output['median']))

if __name__ == '__main__':
    main()
