import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('arc_selfhost', ROOT / 'docs/dogfood/arc-admin/cost/selfhost_trial.py')
trial = importlib.util.module_from_spec(spec)
spec.loader.exec_module(trial)


class SelfhostCostTests(unittest.TestCase):
    def test_failed_command_keeps_exit_status_log_and_elapsed_time(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            result = trial.run(root, 'failed', [sys.executable, '-c', 'print("failed gate"); raise SystemExit(7)'], root, {})
            self.assertEqual(result['exit_code'], 7)
            self.assertGreater(result['elapsed_seconds'], 0)
            self.assertIn('failed gate', (root / 'failed.log').read_text())
            self.assertEqual(json.loads((root / 'failed.json').read_text()), result)

    def test_summary_requires_all_pairs_and_never_converts_failure_to_acceptance(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / 'target/trial'
            evidence = output / 'evidence'
            evidence.mkdir(parents=True)
            argv = ['trial', 'summarize', '--output', str(output)]
            with patch.object(trial, 'ROOT', root), patch.object(sys, 'argv', argv):
                with self.assertRaises(AssertionError):
                    trial.main()
                for sample in (1, 2, 3):
                    for mode, duration, code in [('before', 10, 0), ('shadow', 14, 1)]:
                        directory = evidence / f'{sample}-{mode}'
                        directory.mkdir()
                        trial.write(directory / 'sample.json', {'sample': sample, 'mode': mode,
                            'elapsed_seconds': duration, 'commands': [{'exit_code': code}]})
                trial.main()
            summary = json.loads((evidence / 'summary.json').read_text())
            self.assertFalse(summary['authority_transfer_permitted'])
            for pair in summary['pairs']:
                self.assertEqual(pair['added_seconds'], 4)
                self.assertTrue(pair['before_passed'])
                self.assertFalse(pair['shadow_passed'])

class RetainedCostTests(unittest.TestCase):
    def test_reproduction_rejects_tampered_receipts_and_mixed_ci_identity(self):
        import hashlib
        import copy
        spec = importlib.util.spec_from_file_location('arc_ci_reproduce', ROOT / 'docs/dogfood/arc-admin/cost/ci_reproduce.py')
        reproduce = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(reproduce)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            head = 'a' * 40
            context = {'source_sha': trial.SOURCE_SHA, 'harness_sha': head,
                'ci_identity': {'GITHUB_RUN_ID': '42', 'GITHUB_RUN_ATTEMPT': '1',
                    'GITHUB_SHA': head, 'GITHUB_REPOSITORY': 'musutrade/Harness-Gate',
                    'RUNNER_NAME': 'fixture'}, 'workflow_host': 'fixture', 'cache_protocol': 'fixture'}
            trial.write(root / 'context.json', context)
            trial.write(root / 'actions-run.json', {'id': 42, 'run_attempt': 1, 'head_sha': head,
                'repository': {'full_name': 'musutrade/Harness-Gate'}, 'html_url': 'fixture',
                'created_at': '2026-09-10T00:00:00Z'})
            steps = []
            pairs = []
            for sample in (1, 2, 3):
                for mode, duration, names in [('before', 10, ['arc-full']), ('shadow', 14, ['arc-full', 'harness-full'])]:
                    directory = root / f'{sample}-{mode}'
                    directory.mkdir()
                    commands = [{'name': name, 'elapsed_seconds': 4,
                                 'exit_code': int(name == 'harness-full')} for name in names]
                    for command in commands:
                        trial.write(directory / f'{command["name"]}.json', command)
                    trial.write(directory / 'sample.json', {'sample': sample, 'mode': mode,
                        'commands': commands, 'elapsed_seconds': duration, 'passed': mode == 'before'})
                    steps.append({'name': f'{mode.title()} sample {sample}', 'status': 'completed',
                        'started_at': f'2026-09-10T00:0{len(steps)+1}:00Z',
                        'completed_at': f'2026-09-10T00:0{len(steps)+2}:00Z'})
                inventory = json.loads((ROOT / 'docs/dogfood/arc-admin/inventory.json').read_text())
                labels = [s['label'] for s in inventory['flow']['steps']] + inventory['always_blocking_prelude']
                report = {'profile': 'full', 'scope': {'mode': 'all'}, 'steps': [{'label': label, 'passed': True} for label in labels]}
                for owner in ('arc', 'harness'):
                    directory = root / f'{sample}-shadow/{owner}-reports'
                    directory.mkdir()
                    if owner == 'harness':
                        report.update(source_identity='working-tree:' + trial.SOURCE_SHA,
                                      quality={'status': 'blocked', 'error': 'missing trusted input'}, failures=[])
                    trial.write(directory / 'test_result.json', report)
                pairs.append({'sample': sample, 'before_seconds': 10, 'shadow_seconds': 14, 'added_seconds': 4})
            trial.write(root / 'summary.json', {'pairs': pairs, 'authority_transfer_permitted': False})
            trial.write(root / 'actions-jobs.json', {'jobs': [{'name': 'measure', 'run_id': 42,
                'runner_name': 'fixture', 'labels': ['gh206-measure'], 'runner_id': 3,
                'steps': steps, 'html_url': 'fixture', 'started_at': '2026-09-10T00:01:00Z',
                'completed_at': '2026-09-10T00:07:00Z'}]})
            def seal():
                trial.write(root / 'sha256.json', {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                    for p in root.rglob('*') if p.is_file() and p != root / 'sha256.json'})
            seal()
            result = reproduce.derive(root)
            self.assertEqual(result['job_runner_seconds'], 360)
            self.assertFalse(result['authority_transfer_permitted'])
            self.assertFalse(result['samples'][0]['shadow']['commands_passed'])
            changed = copy.deepcopy(context)
            changed['harness_sha'] = 'b' * 40
            trial.write(root / 'context.json', changed)
            with self.assertRaises(AssertionError):
                reproduce.derive(root)
            seal()
            with self.assertRaises(AssertionError):
                reproduce.derive(root)
