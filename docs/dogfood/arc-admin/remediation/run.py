#!/usr/bin/env python3
"""Rerun unchanged Arc-Admin commands locally; retain failures as evidence."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
spec = importlib.util.spec_from_file_location('trial', HERE.parent / 'cost/selfhost_trial.py')
trial = importlib.util.module_from_spec(spec)
spec.loader.exec_module(trial)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepared', required=True, type=Path)
    parser.add_argument('--harness-gate', required=True, type=Path)
    args = parser.parse_args()
    prepared = args.prepared.resolve()
    assert prepared.is_relative_to(ROOT / 'target')
    source = prepared / 'source'
    evidence = prepared / 'rerun'
    evidence.mkdir()
    binary = args.harness_gate.resolve()
    env = {k: v for k, v in os.environ.items() if k not in trial.REMOVED}
    env.update(CARGO_TARGET_DIR=str(prepared / 'cargo-target'), TMPDIR=str(prepared / 'tmp'),
               npm_config_cache=str(prepared / 'npm-cache'),
               GIT_CEILING_DIRECTORIES=str(prepared / 'tmp'))
    hashes = json.loads((prepared / 'evidence/configuration-hashes.json').read_text())
    assert all(hashlib.sha256((source / p).read_bytes()).hexdigest() == h for p, h in hashes.items())
    assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source, text=True).strip() == trial.SOURCE_SHA
    subprocess.run(['git', 'diff', '--exit-code', 'HEAD'], cwd=source, check=True)
    receipts = []
    prefix = [str(binary), '--project-root', str(source)]
    commands = [('arc', ['cargo', 'flow'])] + [
        (name, prefix + ['--config', config + '/flow.toml'])
        for name, config in [('execution', '.shadow-execution'), ('quality', '.harness-gate')]]
    reports = {}
    for name, command in commands:
        receipts.append(trial.run(evidence, name + '-scope', command + ['scope', '--all'], source, env))
        # Never attribute a previous engine's report to this invocation.
        reports_dir = source / 'codex-audit-pipeline/.codex/reports'
        if reports_dir.exists():
            shutil.rmtree(reports_dir)
        receipts.append(trial.run(evidence, name + '-full', command + [
            'verify', '--profile', 'full', '--all'], source, env))
        destination = evidence / name
        trial.preserve_reports(source, destination)
        report_path = destination / 'test_result.json'
        reports[name] = json.loads(report_path.read_text()) if report_path.exists() else None
    assert all(hashlib.sha256((source / p).read_bytes()).hexdigest() == h for p, h in hashes.items())
    subprocess.run(['git', 'diff', '--exit-code', 'HEAD'], cwd=source, check=True)
    trial.write(evidence / 'receipt.json', {
        'source_sha': trial.SOURCE_SHA,
        'harness_binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'configuration_hashes': hashes, 'tracked_source_unchanged': True,
        'commands': receipts, 'reports_present': {k: v is not None for k, v in reports.items()},
        'authority_transfer_permitted': False,
        'scope': 'Local full-command rerun; not hosted CI or original-topology cost evidence.'})


if __name__ == '__main__':
    main()
