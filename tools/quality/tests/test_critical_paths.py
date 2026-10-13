from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import critical_paths as gate


class RepositoryCriticalPathInventoryTests(unittest.TestCase):
    def test_checked_in_inventory_matches_current_sources_and_assertions(self):
        gate.validate_inventory(tomllib.loads(gate.INVENTORY.read_text()),
                                json.loads(gate.POLICY.read_text()))


class CriticalPathEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.crate = self.base / 'crate'
        (self.crate / 'src').mkdir(parents=True)
        self.source = self.crate / 'src/lib.rs'
        self.source.write_text('fn boundary() {\n    fail_closed();\n}\nfn boundary_test() {\n    assert!(!passed);\n}\n')
        self.identity = {'commit': 'a' * 40, 'target': 'x86_64-unknown-linux-gnu', 'rule': gate.RULE}
        self.policy = json.loads(gate.POLICY.read_text())
        self.inventory = {'version': 2, 'rule': gate.RULE, 'paths': []}
        self.bundle = {'identity': self.identity.copy(), 'source_root': str(self.crate),
                       'sources': gate.source_identity(self.crate), 'runs': {}}
        for index, (name, platforms) in enumerate(self.policy['mandatory'].items()):
            row = {'id': name, 'rule': gate.RULE, 'platforms': platforms, 'platform_note': 'reviewed fixture',
                   'owner': 'test', 'review_date': '2026-09-07', 'observable': 'fails closed',
                   'assertions': ['assert!(!passed);'], 'binary': 'harness-gate', 'test': 'boundary_test',
                   'source': gate.function_binding(self.crate, 'src/lib.rs', 'boundary'),
                   'test_source': gate.function_binding(self.crate, 'src/lib.rs', 'boundary_test'),
                   'probes': [{'line': 2, 'column': 5, 'text': 'fail_closed();'}]}
            self.inventory['paths'].append(row)
            run_id = f'run-{index}'
            (self.base / run_id).mkdir()
            run = {'identity': self.identity.copy(), 'run_id': run_id, 'binary': row['binary'],
                   'test': row['test'], 'clean_exit': 0, 'test_exit': 0, 'coverage_exit': 0,
                   'observable': row['observable'], 'assertions': row['assertions'].copy()}
            for kind in ('nextest', 'coverage'):
                run[kind] = {'identity': self.identity.copy(), 'run_id': run_id,
                             'path': f'{run_id}/{kind}.json'}
            self.bundle['runs'][name] = run
            self.write_artifact(run, 'nextest', '\n'.join(json.dumps(e) for e in [
                {'type': 'test', 'event': 'ok', 'name': 'harness-gate::harness-gate$boundary_test'},
                {'type': 'suite', 'event': 'ok', 'passed': 1, 'failed': 0, 'ignored': 0}]))
            self.write_artifact(run, 'coverage', json.dumps({'type': 'llvm.coverage.json.export', 'data': [{
                'functions': [{'name': 'crate::boundary', 'filenames': [str(self.source)],
                               'regions': [[1, 1, 3, 2, 1, 0, 0, 0], [2, 5, 2, 19, 1, 0, 0, 0]]}]}]}))
        self.seal_inventory()

    def seal_inventory(self):
        self.bundle['inventory_sha256'] = hashlib.sha256(json.dumps(self.inventory, sort_keys=True).encode()).hexdigest()

    def write_artifact(self, run, kind, value):
        path = self.base / run[kind]['path']
        path.write_text(value)
        run[kind]['sha256'] = gate.sha256(path)

    def evaluate(self):
        return gate.evaluate(self.inventory, self.policy, self.bundle, self.base,
                             self.identity['commit'], self.identity['target'], self.crate)

    def assert_fails(self):
        try:
            result = self.evaluate()
        except (ValueError, KeyError, OSError):
            return
        self.assertEqual(result['summary']['status'], 'fail')

    def test_complete_isolated_matrix_passes(self):
        self.assertEqual(self.evaluate()['summary']['percent'], 100)
        self.assertEqual(self.evaluate()['summary']['status'], 'pass')

    def test_deleting_each_mandatory_row_fails_before_percentage(self):
        original = copy.deepcopy(self.inventory)
        for name in self.policy['mandatory']:
            with self.subTest(name=name):
                self.inventory = copy.deepcopy(original)
                self.inventory['paths'] = [r for r in self.inventory['paths'] if r['id'] != name]
                self.seal_inventory()
                with self.assertRaisesRegex(ValueError, 'missing mandatory IDs'):
                    self.evaluate()

    def test_skipped_cancelled_failed_missing_and_extra_tests_fail(self):
        run = next(iter(self.bundle['runs'].values()))
        original = (self.base / run['nextest']['path']).read_text()
        for mutation in ('ignored', 'cancelled', 'failed', 'missing', 'extra', 'wrong-name', 'failed-suite'):
            with self.subTest(mutation=mutation):
                events = [json.loads(line) for line in original.splitlines()]
                if mutation == 'missing':
                    events.pop(0)
                elif mutation == 'extra':
                    events.insert(0, events[0].copy())
                elif mutation == 'wrong-name':
                    events[0]['name'] += '_other'
                elif mutation == 'failed-suite':
                    events.append({'type': 'suite', 'event': 'failed'})
                else:
                    events[0]['event'] = mutation
                self.write_artifact(run, 'nextest', '\n'.join(json.dumps(e) for e in events))
                self.assert_fails()

    def test_stale_and_mixed_commits_targets_and_rules_fail(self):
        original = copy.deepcopy(self.bundle)
        for location in ('bundle', 'run', 'nextest', 'coverage'):
            for field in ('commit', 'target', 'rule'):
                with self.subTest(location=location, field=field):
                    self.bundle = copy.deepcopy(original)
                    run = next(iter(self.bundle['runs'].values()))
                    item = self.bundle if location == 'bundle' else run if location == 'run' else run[location]
                    item['identity'][field] = 'foreign'
                    self.assert_fails()

    def test_other_test_cannot_lend_coverage_or_reuse_run(self):
        runs = list(self.bundle['runs'].values())
        runs[0]['coverage'] = copy.deepcopy(runs[1]['coverage'])
        self.assert_fails()
        runs[0]['coverage']['run_id'] = runs[0]['run_id']
        self.assert_fails()

    def test_hit_in_outer_function_cannot_cover_zero_hit_failure_region(self):
        run = next(iter(self.bundle['runs'].values()))
        data = json.loads((self.base / run['coverage']['path']).read_text())
        data['data'][0]['functions'][0]['regions'][1][4] = 0
        self.write_artifact(run, 'coverage', json.dumps(data))
        self.assert_fails()

    def test_other_source_region_cannot_cover_required_function(self):
        run = next(iter(self.bundle['runs'].values()))
        data = json.loads((self.base / run['coverage']['path']).read_text())
        data['data'][0]['functions'][0]['regions'] = [[4, 1, 6, 2, 100, 0, 0, 0]]
        self.write_artifact(run, 'coverage', json.dumps(data))
        self.assert_fails()

    def test_missing_or_modified_coverage_fails(self):
        run = next(iter(self.bundle['runs'].values()))
        path = self.base / run['coverage']['path']
        path.write_text('{}')
        self.assert_fails()
        path.unlink()
        self.assert_fails()

    def test_nonzero_test_and_coverage_commands_fail(self):
        run = next(iter(self.bundle['runs'].values()))
        for field in ('clean_exit', 'test_exit', 'coverage_exit'):
            run[field] = 1
            self.assert_fails()
            run[field] = 0

    def test_source_bytes_must_match_declared_git_commit(self):
        with patch.object(gate, 'source_identity', return_value={'src/lib.rs': 'wrong'}), \
                patch.object(gate.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, b'source')):
            with self.assertRaisesRegex(ValueError, 'source snapshot differs from commit'):
                gate.require_committed_sources('a' * 40)

    def test_moved_symbol_and_changed_unbound_source_fail(self):
        self.source.write_text('\n' + self.source.read_text())
        self.assert_fails()
        self.source.write_text(self.source.read_text()[1:])
        (self.crate / 'src/helper.rs').write_text('fn changed() {}')
        self.assert_fails()

    def test_degraded_observable_assertions_fail_even_if_rebound(self):
        self.source.write_text(self.source.read_text().replace('assert!(!passed);', 'assert!(true);'))
        for row in self.inventory['paths']:
            row['source'] = gate.function_binding(self.crate, 'src/lib.rs', 'boundary')
            row['test_source'] = gate.function_binding(self.crate, 'src/lib.rs', 'boundary_test')
        self.bundle['sources'] = gate.source_identity(self.crate)
        self.seal_inventory()
        with self.assertRaisesRegex(ValueError, 'degraded observable assertion'):
            self.evaluate()

    def test_degraded_run_observable_and_missing_mandatory_run_fail(self):
        run = next(iter(self.bundle['runs'].values()))
        run['assertions'] = ['assert!(true);']
        self.assert_fails()
        self.bundle['runs'].pop(next(iter(self.bundle['runs'])))
        self.assert_fails()

    def test_cli_exits_nonzero_and_records_fail_closed_error(self):
        evidence = self.base / 'bundle.json'
        output = self.base / 'result.json'
        original_inventory = copy.deepcopy(self.inventory)
        original_bundle = copy.deepcopy(self.bundle)
        evaluate = gate.evaluate
        for mutation in ('valid', 'deleted-row', 'moved-symbol', 'skipped', 'mixed-commit'):
            with self.subTest(mutation=mutation):
                self.inventory = copy.deepcopy(original_inventory)
                self.bundle = copy.deepcopy(original_bundle)
                if mutation == 'deleted-row':
                    self.inventory['paths'].pop(0)
                elif mutation == 'moved-symbol':
                    self.inventory['paths'][0]['source']['start'] += 1
                elif mutation == 'skipped':
                    run = next(iter(self.bundle['runs'].values()))
                    events = [json.loads(line) for line in (self.base / run['nextest']['path']).read_text().splitlines()]
                    events[0]['event'] = 'ignored'
                    self.write_artifact(run, 'nextest', '\n'.join(json.dumps(e) for e in events))
                elif mutation == 'mixed-commit':
                    next(iter(self.bundle['runs'].values()))['coverage']['identity']['commit'] = 'b' * 40
                self.seal_inventory()
                evidence.write_text(json.dumps(self.bundle))
                # Only repository inputs are replaced; exercise the actual CLI,
                # validation, artifact loading, coverage evaluation and exit path.
                with patch.object(sys, 'argv', ['critical_paths.py', '--evidence', str(evidence), '--output', str(output)]), \
                     patch.object(gate.tomllib, 'loads', return_value=self.inventory), \
                     patch.object(gate, 'metadata', return_value=self.identity), \
                     patch.object(gate, 'git_sha', return_value=self.identity['commit']), \
                     patch.object(gate, 'require_committed_sources'), \
                     patch.object(gate, 'evaluate', side_effect=lambda *args: evaluate(*args, crate=self.crate)):
                    self.assertEqual(gate.main(), 0 if mutation == 'valid' else 1)
                self.assertEqual(json.loads(output.read_text())['summary']['status'],
                                 'pass' if mutation == 'valid' else 'fail')

    def test_threshold_cannot_be_weakened_or_nan(self):
        for threshold in (94, float('nan'), float('inf')):
            with self.subTest(threshold=threshold), self.assertRaises(ValueError):
                gate.run(self.base / 'out.json', self.base / 'missing', threshold)

    def test_mandatory_failure_overrides_95_percent(self):
        template = copy.deepcopy(self.inventory['paths'][-1])
        original = copy.deepcopy(self.bundle['runs'][template['id']])
        for index in range(20):
            row = copy.deepcopy(template)
            row['id'] = f'optional-{index}'
            self.inventory['paths'].append(row)
            run = copy.deepcopy(original)
            run['run_id'] = row['id']
            (self.base / run['run_id']).mkdir()
            for kind in ('nextest', 'coverage'):
                run[kind]['run_id'] = run['run_id']
                run[kind]['path'] = f"{run['run_id']}/{kind}.json"
                self.write_artifact(run, kind, (self.base / original[kind]['path']).read_text())
            self.bundle['runs'][row['id']] = run
        self.bundle['runs'].pop(next(iter(self.policy['mandatory'])))
        self.seal_inventory()
        result = self.evaluate()
        self.assertGreaterEqual(result['summary']['percent'], 95)
        self.assertEqual(result['summary']['status'], 'fail')

    def test_mandatory_applicability_cannot_be_silently_narrowed(self):
        self.inventory['paths'][0]['platforms'] = ['windows']
        self.seal_inventory()
        self.assert_fails()


if __name__ == '__main__':
    unittest.main()
