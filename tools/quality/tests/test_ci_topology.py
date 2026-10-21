"""Frozen event contract independent of the aggregate implementation constants."""
import copy
import contextlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_quality
from quality_common import ROOT

CONTRACT = json.loads((ROOT / 'tools/quality/fixtures/ci-topology.json').read_text())


def required_jobs(event):
    return [key for key, job in CONTRACT['jobs'].items()
            if event in job['required_events'] or job.get('aggregate_required')]


class TopologyTests(unittest.TestCase):
    def test_all_frozen_required_children_fail_closed(self):
        for event in CONTRACT['events']:
            required = required_jobs(event)
            needs = {key: {'result': 'success' if key in required else 'skipped'}
                     for key in CONTRACT['jobs']['quality-required']['needs']}
            self.assertEqual(ci_quality.aggregate(event, needs), [])
            for child in required:
                for state in ('missing', 'missing-result', 'failure', 'cancelled', 'skipped',
                              '', 'unknown', None, True, ['success'], {'status': 'success'}):
                    with self.subTest(event=event, child=child, state=state):
                        bad = copy.deepcopy(needs)
                        if state == 'missing':
                            del bad[child]
                        elif state == 'missing-result':
                            bad[child] = {}
                        else:
                            bad[child]['result'] = state
                        self.assertEqual(ci_quality.aggregate(event, bad), [child])

    def test_push_only_children_are_ignored_only_on_pull_requests(self):
        push_only = [key for key, job in CONTRACT['jobs'].items()
                     if job['required_events'] == ['push']]
        self.assertEqual(set(push_only), set(ci_quality.PUSH_ONLY))
        for child in push_only:
            for state in ('missing', 'skipped', 'failure', 'cancelled', 'success'):
                with self.subTest(child=child, state=state):
                    needs = {key: {'result': 'success'}
                             for key in CONTRACT['jobs']['quality-required']['needs']}
                    if state == 'missing':
                        del needs[child]
                    else:
                        needs[child]['result'] = state
                    self.assertEqual(ci_quality.aggregate('pull_request', needs), [])
                    self.assertEqual(ci_quality.aggregate('push', needs),
                                     [] if state == 'success' else [child])

    def test_malformed_required_child_fails_closed(self):
        for event in CONTRACT['events']:
            for child in required_jobs(event):
                for value in (None, [], 'success', True, 1):
                    with self.subTest(event=event, child=child, value=value):
                        needs = {key: {'result': 'success'}
                                 for key in CONTRACT['jobs']['quality-required']['needs']}
                        needs[child] = value
                        self.assertEqual(ci_quality.aggregate(event, needs), [child])

    def test_cli_only_evaluates_results_without_collection_or_processes(self):
        needs = {key: {'result': 'success'} for key, job in CONTRACT['jobs'].items()
                 if 'pull_request' in job['required_events']}
        needs['classify'] = {'result': 'success'}
        cases = [('pull_request', json.dumps(needs), 0),
                 ('push', json.dumps(needs), 1), ('unknown', json.dumps(needs), 1)]
        cases.extend(('pull_request', value, 1)
                     for value in ('{}', 'null', '[]', 'true', '1', '"success"', '{'))
        for event, payload, expected in cases:
            with self.subTest(event=event, payload=payload), contextlib.ExitStack() as stack:
                for target in ('ci_quality.Collector', 'ci_quality.verify',
                               'ci_quality.metadata', 'ci_quality.write_json',
                               'subprocess.Popen'):
                    stack.enter_context(patch(target, side_effect=AssertionError(target)))
                stack.enter_context(patch.dict('os.environ',
                                              EVENT_NAME=event, NEEDS_JSON=payload))
                stack.enter_context(patch.object(sys, 'argv', ['ci_quality.py', 'aggregate']))
                stdout, stderr = io.StringIO(), io.StringIO()
                stack.enter_context(contextlib.redirect_stdout(stdout))
                stack.enter_context(contextlib.redirect_stderr(stderr))
                self.assertEqual(ci_quality.main(), expected)
                self.assertEqual(stdout.getvalue(), '')
                self.assertLess(len(stderr.getvalue()), 1024)
                self.assertEqual(bool(stderr.getvalue()), bool(expected))

    def test_aggregate_step_allowlist_prohibits_heavy_work(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        body = re.split(r'^  [a-z][a-z-]+:\n',
                        workflow.split('  quality-required:\n')[1], maxsplit=1, flags=re.M)[0]
        steps = body.split('    steps:\n', 1)[1]
        # Compare all step content: extra actions, multiline commands, conditional
        # evaluation and shell error suppression must all require policy review.
        expected = '''
      - uses: actions/checkout@fbc6f3992d24b796d5a048ff273f7fcc4a7b6c09 # v5
      - name: Install Python
        uses: actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97 # v7
        with:
          python-version: '3.x'
      - name: Require every quality gate
        env:
          EVENT_NAME: ${{ github.event_name }}
          NEEDS_JSON: ${{ toJSON(needs) }}
          REUSE_VERIFIED: ${{ needs.classify.outputs.reuse_verified }}
          RUN_FULL: ${{ needs.classify.outputs.run_full }}
          RUN_EVIDENCE: ${{ needs.classify.outputs.run_evidence }}
          RUN_NATIVE: ${{ needs.classify.outputs.run_native }}
        run: python3 tools/quality/ci_quality.py aggregate
'''
        def significant_lines(value):
            return [line for line in value.splitlines()
                    if line.strip() and not line.lstrip().startswith('#')]
        self.assertEqual(significant_lines(steps), significant_lines(expected))

    def test_inheritance_receipt_precedes_repository_classification(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        classify = workflow.split('  classify:\n', 1)[1].split('  test:\n', 1)[0]
        names = re.findall(r'^      - name: (.+)$', classify, re.M)
        self.assertEqual(names, ['Install Python', 'Record PR equivalence receipt',
                                'Upload PR equivalence receipt',
                                'Verify prior PR checks for equivalent push',
                                'Classify changed paths'])
        self.assertIn('actions: read', classify)
        self.assertIn('pull-requests: read', classify)
        self.assertNotRegex(classify, r': write')
        self.assertIn('python3 -I tools/quality/ci_reuse.py record', classify)
        self.assertIn('python3 -I tools/quality/ci_reuse.py probe', classify)
        self.assertIn("steps.reuse.outputs.reuse_verified || 'false'", classify)
        record, probe = classify.split('      - name: Verify prior PR checks for equivalent push')
        self.assertEqual(record.count("if: github.event_name == 'pull_request'"), 2)
        self.assertIn("if: github.event_name == 'push'", probe)
        self.assertIn('retention-days: 3', record)
        self.assertEqual(re.findall(r'^        run: (.+)$', record, re.M), ['>-'])

    def test_cross_platform_pr_failure_blocks_cli(self):
        needs = {key: {'result': 'success'} for key, job in CONTRACT['jobs'].items()
                 if 'pull_request' in job['required_events']}
        needs['test-cross-platform']['result'] = 'failure'
        result = subprocess.run([sys.executable, str(Path(ci_quality.__file__)),
                                 'aggregate', '--event', 'pull_request', '--needs',
                                 json.dumps(needs)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn('test-cross-platform', result.stderr)

    def test_workflow_matches_frozen_event_and_check_contract(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('name: CI\n', workflow)
        self.assertIn('  push:\n    branches: [ main ]', workflow)
        self.assertIn('  pull_request:\n    branches: [ main ]', workflow)
        parts = re.split(r'^  ([a-z][a-z-]+):\n', workflow.split('jobs:\n')[1], flags=re.M)
        jobs = dict(zip(parts[1::2], parts[2::2]))
        self.assertEqual(set(jobs), set(CONTRACT['jobs']))
        for key, expected in CONTRACT['jobs'].items():
            with self.subTest(job=key):
                body = jobs[key]
                self.assertEqual(re.search(r'^    name: (.+)$', body, re.M)[1], expected['name'])
                condition = re.search(r'^    if: (.+)$', body, re.M)
                self.assertEqual(condition[1] if condition else None, expected.get('condition'))
                needs = re.search(r'^    needs:\s*\[([^]]+)\]', body, re.M)
                self.assertEqual(re.findall(r'[a-z][a-z-]+', needs[1]) if needs else [],
                                 expected['needs'])
                matrix = re.search(r'^        os: \[(.+)\]', body, re.M)
                labels = matrix[1].split(', ') if matrix else [
                    re.search(r'^    runs-on: (.+)$', body, re.M)[1]]
                self.assertEqual(labels, expected['runner_labels'])
                if expected['required_events']:
                    self.assertNotIn('continue-on-error:', body)
        native = jobs['test-cross-platform']
        self.assertIn('fail-fast: false', native)
        self.assertIn('shell: bash', native)
        self.assertIn('cargo nextest run --manifest-path tools/harness-gate/Cargo.toml --locked \\', native)
        self.assertIn('--no-fail-fast --partition "count:${CI_SHARD}/${CI_PARTITIONS}"', native)
        self.assertIn('-E "$extra"', native)
        native_steps = native.split('    steps:\n', 1)[1]
        steps = native.split('    steps:\n', 1)[1]
        allowed = {
            'Repeat final cancellation boundary on Windows': "matrix.os == 'windows-latest' && matrix.shard == 1",
            'Retain Windows cancellation samples': "always() && matrix.os == 'windows-latest' && matrix.shard == 1",
        }
        for name, condition in allowed.items():
            blocks = re.findall(r'      - name: ' + re.escape(name) + r'\n(.*?)(?=\n      - |\Z)', steps, re.S)
            self.assertEqual(len(blocks), 1)
            self.assertEqual(re.findall(r'^\s+if: (.+)$', blocks[0], re.M), [condition])
            steps = steps.replace(blocks[0], '', 1)
        self.assertNotRegex(steps, re.compile(r'^\s+if:', re.M))
        aggregate = jobs['quality-required']
        self.assertEqual(re.findall(r'^        run: (.+)$', aggregate, re.M),
                         ['python3 tools/quality/ci_quality.py aggregate'])
        self.assertIn('EVENT_NAME: ${{ github.event_name }}', aggregate)
        self.assertIn('NEEDS_JSON: ${{ toJSON(needs) }}', aggregate)
        self.assertIn('RUN_FULL: ${{ needs.classify.outputs.run_full }}', aggregate)


class SupplementalCadenceTests(unittest.TestCase):
    def test_daily_manual_workflow_preserves_measurement_contract(self):
        workflow = (ROOT / '.github/workflows/quality-trends.yml').read_text()
        self.assertIn("cron: '43 2 * * *'", workflow)
        self.assertIn('  workflow_dispatch:', workflow)
        self.assertNotRegex(workflow, re.compile(r'^  (push|pull_request):', re.M))
        self.assertIn('contents: read', workflow)
        self.assertNotRegex(workflow, r'contents: write|pull-requests: write|continue-on-error:')
        self.assertIn('os: [ubuntu-latest, macos-latest, windows-latest]', workflow)
        self.assertIn('python tools/quality/benchmarks.py --output target/quality/benchmarks.json --samples 5', workflow)
        self.assertIn('cargo tarpaulin --manifest-path tools/harness-gate/Cargo.toml --locked --engine llvm --timeout 300 --out Xml', workflow)
        self.assertEqual(workflow.count("cache-target: 'false'"), 2)
        self.assertIn("profiles: 'release-small,test;default;cold'", workflow)
        self.assertIn('fail-fast: false', workflow)
        self.assertNotRegex(workflow, r'\|\|\s*true|baseline_delivery.py|create-pull-request')
        jobs = re.split(r'^  ([a-z][a-z-]+):\n', workflow.split('jobs:\n')[1], flags=re.M)
        jobs = dict(zip(jobs[1::2], jobs[2::2]))
        self.assertEqual(set(jobs), {'coverage', 'quality-baseline'})
        for body in jobs.values():
            # Capture must run and fail normally; only artifact retention uses always().
            before_upload = body.split('      - name: Upload')[0]
            self.assertNotIn('if:', before_upload)

    def test_supplemental_jobs_are_separate_from_per_change_required_checks(self):
        previous = json.loads((ROOT / 'tools/quality/fixtures/ci-topology-pre-cadence.json').read_text())
        self.assertEqual(set(previous['jobs']) - set(CONTRACT['jobs']), {'coverage', 'quality-baseline'})
        added = set(CONTRACT['jobs']) - set(previous['jobs'])
        # Post-cadence additions are the split-collection and archive-backed
        # fast-coverage workers.
        self.assertEqual(added, {'quality-fast', 'quality-fast-build',
                                 'quality-fast-shard', 'quality-risk', 'quality-matrix'})
        for name in sorted(added):
            row = CONTRACT['jobs'][name]
            self.assertEqual(row['required_events'], [], name)
            expected_needs = (['classify', 'quality-fast-build']
                              if name == 'quality-fast-shard'
                              else ['classify', 'quality-fast-build', 'quality-fast-shard']
                              if name == 'quality-fast'
                              else ['classify'])
            self.assertEqual(row['needs'], expected_needs, name)
            self.assertEqual(row['runner_labels'], ['ubuntu-latest'], name)
            self.assertEqual(row['events'], ['pull_request', 'push'], name)
            self.assertEqual(row['condition'],
                             "${{ needs.classify.outputs.run_full == 'true' && needs.classify.outputs.run_evidence != 'false' && needs.classify.outputs.reuse_verified != 'true' }}", name)
        for name, row in CONTRACT['jobs'].items():
            if name == 'quality-required' or name in added:
                continue
            expected = copy.deepcopy(previous['jobs'][name])
            if name in ('test', 'test-cross-platform', 'quality-coverage', 'quality-generic-shadow'):
                expected['condition'] = expected['condition'].replace(
                    ' }}', " && needs.classify.outputs.reuse_verified != 'true' }}")
            if name in ('quality-coverage', 'quality-generic-shadow'):
                expected['condition'] = expected['condition'].replace(
                    "needs.classify.outputs.run_full == 'true'",
                    "needs.classify.outputs.run_full == 'true' && needs.classify.outputs.run_evidence != 'false'")
            if name == 'quality-coverage':
                expected['needs'] = ['classify', 'quality-fast', 'quality-risk', 'quality-matrix']
                expected['outcome'] = (
                    'Merge the fast, risk and matrix shard candidates, require the complete'
                    ' four-stage evidence set, verify hashes, and always retain the sealed candidate.')
            if name in ('test-cross-platform', 'build-cross-platform', 'quality-contracts-cross-platform'):
                expected['condition'] = expected['condition'].replace(
                    "needs.classify.outputs.run_full == 'true'",
                    "needs.classify.outputs.run_full == 'true' && needs.classify.outputs.run_native != 'false'").replace(
                    "github.event_name == 'push'", "github.event_name != 'pull_request'")
            if name in ('test', 'test-cross-platform'):
                # Matrix shards retain the original shard 1 check names while
                # additional shards get explicit suffixed display names.
                expected['name'] = row['name']
                expected['check_names'] = row['check_names']
                expected['outcome'] = row['outcome']
            self.assertEqual(row, expected)
        self.assertEqual(set(previous['jobs']['quality-required']['needs']) -
                         set(CONTRACT['jobs']['quality-required']['needs']),
                         {'coverage', 'quality-baseline'})


class ClassifyAndSelectionTests(unittest.TestCase):
    def test_pure_document_paths_exclude_inputs_fixtures_and_protocols(self):
        for path in ('README.md', 'CHANGELOG.md', 'docs/development.md', 'docs/adr/0001-x.md',
                     'docs/releases/0.4.8.zh-CN.md', 'docs/engineering-policy.md'):
            self.assertTrue(ci_quality.is_pure_document(path), path)
        for path in ('docs/dogfood/arc-admin/import/flow.toml', 'docs/benchmarks/phase-1/README.md',
                     'docs/quality/gh-151/README.md', 'docs/quality/schema-ownership.json',
                     'schema/README.md', 'schema/flow.schema.json',
                     'tools/quality/fixtures/ci-topology.json', 'tools/quality/ci_quality.py',
                     'presets/rust-api.flow.toml', '.github/workflows/ci.yml'):
            self.assertFalse(ci_quality.is_pure_document(path), path)

    def test_classify_falls_back_to_full_without_a_resolvable_diff(self):
        for base, head in (('', ''), ('0' * 40, 'a' * 40), ('a' * 40, '')):
            with self.subTest(base=base, head=head):
                self.assertEqual(ci_quality.classify(base, head),
                                 {'docs_only': False, 'run_full': True, 'files': 0})

    def test_classify_docs_only_and_mixed_changes(self):
        with patch.object(ci_quality, 'changed_paths',
                          return_value=['README.md', 'docs/adr/0001-x.md']):
            self.assertEqual(ci_quality.classify('a' * 40, 'b' * 40),
                             {'docs_only': True, 'run_full': False, 'files': 2})
        with patch.object(ci_quality, 'changed_paths',
                          return_value=['plugins/README.md', 'tools/quality/ci_quality.py']):
            self.assertEqual(ci_quality.classify('a' * 40, 'b' * 40),
                             {'docs_only': False, 'run_full': True, 'files': 2})

    def test_pure_docs_select_light_checks_and_skip_heavy_jobs(self):
        needs = {key: {'result': 'skipped'}
                 for key in CONTRACT['jobs']['quality-required']['needs']}
        needs['classify'] = {'result': 'success'}
        needs['docs-consistency'] = {'result': 'success'}
        for event in ('pull_request', 'push'):
            with self.subTest(event=event):
                self.assertEqual(ci_quality.aggregate(event, needs, full=False), [])
        for child, state in (('test', 'failure'), ('docs-consistency', 'failure'),
                             ('classify', 'skipped')):
            with self.subTest(child=child):
                bad = {key: dict(value) for key, value in needs.items()}
                bad[child]['result'] = state
                self.assertEqual(ci_quality.aggregate('pull_request', bad, full=False), [child])
        missing = {key: dict(value) for key, value in needs.items()}
        del missing['test']
        self.assertEqual(ci_quality.aggregate('pull_request', missing, full=False), ['test'])

    def test_cli_run_full_selects_docs_only_or_full_conservatively(self):
        full = {key: {'result': 'success'}
                for key in CONTRACT['jobs']['quality-required']['needs']}
        docs = {key: {'result': 'skipped'}
                for key in CONTRACT['jobs']['quality-required']['needs']}
        docs['classify'] = {'result': 'success'}
        docs['docs-consistency'] = {'result': 'success'}
        for run_full, payload, expected in (
                ('true', full, 0),
                ('false', docs, 0),
                ('', docs, 1),
                ('yes', docs, 1),
                ('true', docs, 1)):
            with self.subTest(run_full=run_full):
                result = subprocess.run(
                    [sys.executable, str(Path(ci_quality.__file__)), 'aggregate',
                     '--event', 'pull_request', '--needs', json.dumps(payload)],
                    capture_output=True, text=True,
                    env={**os.environ, 'RUN_FULL': run_full})
                self.assertEqual(result.returncode, expected, result.stderr)


class ImpactRoutingTests(unittest.TestCase):
    def test_periodic_workflow_forces_full_and_shares_certified_base(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn("cron: '17 2 * * *'", workflow)
        self.assertIn('  workflow_dispatch:', workflow)
        self.assertIn('ci_quality.py certified-base --head-sha "$HEAD"', workflow)
        self.assertEqual(workflow.count('BASE_SHA: ${{ needs.classify.outputs.base_sha }}'), 5)
        self.assertIn('base_sha: ${{ steps.classify.outputs.base_sha }}', workflow)
        self.assertEqual(CONTRACT['full_validation_events'], ['schedule', 'workflow_dispatch'])

    def test_known_paths_and_platform_markers(self):
        cases = [(['tools/harness-gate/src/config/parser.rs'], False, False),
                 (['tools/harness-gate/src/process/capture.rs'], False, True),
                 (['tools/harness-gate/tests/failure.rs'], False, True),
                 (['tools/quality/ci_quality.py'], True, True),
                 (['tools/quality/ci_reuse.py'], True, True),
                 (['.github/workflows/ci.yml'], True, True),
                 (['tools/harness-gate/Cargo.toml'], True, True),
                 (['tools/harness-gate/fixtures/input.json'], True, True),
                 (['schema/flow.schema.json'], True, True),
                 (['unknown'], True, True), ([], True, True),
                 (['tools/harness-gate/src/config/parser.rs', 'old/unknown.rs'], True, True)]
        for paths, evidence, native in cases:
            with self.subTest(paths=paths), patch.object(ci_quality, 'changed_paths', return_value=paths), patch.object(ci_quality.subprocess, 'check_output', return_value='normal diff'):
                self.assertEqual(ci_quality.impact('a'*40, 'b'*40, 'pull_request'),
                                 {'run_evidence': evidence, 'run_native': native})
        for marker in ('cfg(windows)', 'cfg(unix)', 'std::os', 'unsafe', 'target_os'):
            with patch.object(ci_quality, 'changed_paths', return_value=cases[0][0]), patch.object(ci_quality.subprocess, 'check_output', return_value=marker):
                self.assertTrue(ci_quality.impact('a'*40, 'b'*40, 'push')['run_native'])

    def test_skips_are_explicit_and_periodic_runs_cannot_defer(self):
        required = ci_quality.COMMON + ci_quality.PUSH_ONLY
        needs = {name: {'result': 'success'} for name in required}
        deferred = {'quality-coverage', 'test-cross-platform', *ci_quality.PUSH_ONLY}
        for name in deferred:
            needs[name]['result'] = 'skipped'
        self.assertEqual(ci_quality.aggregate('push', needs, evidence=False, native=False), [])
        for name in required:
            for state in ('failure', 'cancelled', None):
                bad = copy.deepcopy(needs)
                bad[name] = {'result': state}
                self.assertIn(name, ci_quality.aggregate('push', bad, evidence=False, native=False))
            bad = copy.deepcopy(needs)
            del bad[name]
            self.assertIn(name, ci_quality.aggregate('push', bad, evidence=False, native=False))
        bad = copy.deepcopy(needs)
        bad['test']['result'] = 'skipped'
        self.assertIn('test', ci_quality.aggregate('push', bad, evidence=False, native=False))
        for event in ('schedule', 'workflow_dispatch'):
            self.assertEqual(ci_quality.impact('a'*40, 'b'*40, event), {'run_evidence': True, 'run_native': True})
            with self.assertRaises(ValueError):
                ci_quality.aggregate(event, needs, evidence=False, native=False)
            self.assertTrue(ci_quality.aggregate(event, needs))

    def test_periodic_base_ignores_skipped_collection_and_failed_lookup(self):
        runs = {'workflow_runs': [dict(id=3, run_attempt=1, event='push', head_sha='d'*40),
                                  dict(id=2, run_attempt=1, event='schedule', head_sha='b'*40),
                                  dict(id=1, run_attempt=1, event='schedule', head_sha='a'*40)]}
        answers = [json.dumps(runs), json.dumps({'jobs': [{'name': 'Quality Coverage and Critical Paths', 'conclusion': 'skipped'}]}),
                   json.dumps({'jobs': [{'name': 'Quality Coverage and Critical Paths', 'conclusion': 'success'}]})]
        with patch.dict(os.environ, GITHUB_REPOSITORY='fixture/repo'), patch.object(ci_quality.subprocess, 'check_output', side_effect=answers), patch.object(ci_quality.subprocess, 'run') as run:
            run.return_value.returncode = 0
            self.assertEqual(ci_quality.certified_base('c'*40), 'a'*40)
        with patch.dict(os.environ, GITHUB_REPOSITORY='fixture/repo'), patch.object(ci_quality.subprocess, 'check_output', return_value='{"workflow_runs": []}'):
            with self.assertRaises(ValueError):
                ci_quality.certified_base('c'*40)


if __name__ == '__main__':
    unittest.main()
