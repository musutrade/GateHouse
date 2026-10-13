#!/usr/bin/env python3
"""Replay command-boundary faults in disposable fixtures inside this workspace."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(binary, output):
    imported = ROOT.parent / 'import/flow.toml'
    inventory = tomllib.loads(imported.read_text())
    ids = ['frontend.e2e', 'backend.tests', 'frontend.fullstack-smoke']
    steps = {s['id']: s for s in inventory['steps'] if s['id'] in ids}
    before = {str(p.relative_to(ROOT.parent)): digest(p)
              for directory in ['sources', 'import', 'quality']
              for p in (ROOT.parent / directory).rglob('*') if p.is_file()}
    scratch = REPO / 'target/quality/gh-205'
    scratch.mkdir(parents=True, exist_ok=True)
    cases = []
    for step_id in ids:
        for status in [0, 7]:
            with tempfile.TemporaryDirectory(prefix='command-', dir=scratch) as directory:
                root = Path(directory)
                def invoke(*args):
                    command = [str(binary), '--project-root', str(root), *args]
                    result = subprocess.run(command, capture_output=True, text=True, timeout=60)
                    return dict(command=command, exit_code=result.returncode,
                                stdout=result.stdout, stderr=result.stderr)
                assert invoke('init', '--preset', 'generic')['exit_code'] == 0
                subprocess.run(['git', 'init', '-q', str(root)], check=True)
                # Replace only the executable boundary in this isolated fixture.
                # No browser, API assertion, service topology or test logic is copied.
                flow = f'''version = 2
[project]
name = "arc-admin-negative-boundary"
default_profile = "full"
hook_profile = "full"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"
[scope]
unmatched = "all"
[[scope.rules]]
patterns = ["**"]
components = ["{steps[step_id]['component']}"]
[policy]
required_steps = ["{step_id}"]
[[steps]]
id = "{step_id}"
label = "controlled command boundary"
component = "{steps[step_id]['component']}"
profiles = ["full"]
program = "sh"
args = ["boundary.sh"]
cwd = "{{root}}"
log = "boundary.log"
timeout_secs = 20
'''
                (root / '.harness-gate/flow.toml').write_text(flow)
                script = root / 'boundary.sh'
                script.write_text(f'echo controlled-project-command\nexit {status}\n')
                source_hash = digest(script)
                check = invoke('config', 'check')
                assert check['exit_code'] == 0, check
                receipt = invoke('verify', '--profile', 'full', '--all')
                assert receipt['exit_code'] == (1 if status else 0), receipt
                report = json.loads((root / '.harness-gate/reports/test_result.json').read_text())
                assert report['passed'] == (status == 0)
                step = next(s for s in report['steps'] if s['step_id'] == step_id)
                assert step['passed'] == (status == 0)
                log = Path(step['log']).read_text()
                assert 'controlled-project-command' in log
                assert report['evidence_complete']
                sealed = Path(report['report_directory'])
                manifest = json.loads((sealed / 'manifest.json').read_text())
                for artifact in manifest['artifacts']:
                    path = sealed / artifact['path']
                    assert digest(path) == artifact['sha256']
                    assert path.stat().st_size == artifact['size_bytes']
                assert source_hash == digest(script)
                # CI is absent in the pinned import. Verify rejection, never label it PASS.
                missing_ci = invoke('verify', '--profile', 'ci', '--all')
                assert missing_ci['exit_code'] != 0, missing_ci
                assert json.loads((root / '.harness-gate/reports/test_result.json').read_text()) == report
                case = dict(id=f'{step_id}-{status}', injected_exit_code=status,
                            expected_exit_code=1 if status else 0, original_step=steps[step_id],
                            flow=flow, script=script.read_text(), config_check=check,
                            receipt=receipt, report=report, log=log, manifest=manifest,
                            absent_ci=missing_ci, source_unchanged=True)
                cases.append(json.loads(json.dumps(case).replace(str(root), '<fixture>').replace(str(REPO), '<workspace>')))
            assert not root.exists()
    assert all(digest(ROOT.parent / p) == sha for p, sha in before.items())
    output.mkdir(parents=True, exist_ok=True)
    (output / 'commands.json').write_text(json.dumps(dict(
        schema='arc-admin-negative-commands/v1', imported_flow_sha256=digest(imported),
        frozen_source_hashes=before, frozen_sources_unchanged=True,
        disposable_fixtures_removed=True, cases=cases), indent=2, sort_keys=True) + '\n')
    print(f'{len(cases)} command controls/negatives passed; disposable fixtures removed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--harness-gate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    run(args.harness_gate.resolve(), args.output.resolve())
