import copy
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_quality as gate
from quality_common import ROOT, sha256


class SnapshotTests(unittest.TestCase):
    def test_legacy_schedules_fresh_tests_and_production_only_projects_their_exports(self):
        collector = object.__new__(gate.Collector)
        collector.directory = Path('/fresh-candidate')
        with patch.object(collector, 'python') as command:
            collector.legacy()
            collector.production()
        self.assertEqual(command.call_args_list[0].args,
                         ('legacy', 'coverage.py', '--test-runner', 'nextest',
                          '--output', collector.directory / 'coverage.json'))
        self.assertEqual(command.call_args_list[1].args,
                         ('production', 'coverage.py', '--production', '--raw',
                          collector.directory / 'coverage.raw.json', '--lcov',
                          collector.directory / 'coverage.lcov', '--output',
                          collector.directory / 'production.json'))

    def test_documentation_fixtures_follow_measured_commit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, stderr=subprocess.PIPE,
                                               text=True).strip()

            git('init')
            git('config', 'user.name', 'Snapshot Test')
            git('config', 'user.email', 'snapshot@example.invalid')
            paths = ('tools/harness-gate/tests/import_test.rs', 'tools/quality/fixture.json',
                     'schema/fixture.json', 'docs/dogfood/arc-admin/sources/.arc-flow/flow.toml.txt',
                     'docs/dogfood/arc-admin/import/flow.toml',
                     'docs/dogfood/arc-admin/import/flow.import.json')
            for path in paths:
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text('base fixture\n')
            historical = root / 'docs/quality/old-run/evidence.tar.gz'
            historical.parent.mkdir(parents=True)
            historical.write_bytes(b'historical evidence, not a test input')
            git('add', '.')
            git('commit', '-m', 'base fixtures')
            base = git('rev-parse', 'HEAD')
            for path in paths:
                (root / path).write_text('head fixture\n')
            git('commit', '-am', 'head fixtures')
            head = git('rev-parse', 'HEAD')
            for path in paths:
                (root / path).unlink()
            with patch.object(gate, 'ROOT', root):
                collector = gate.Collector(root / 'candidate', base, head, 'snapshot-test')
                for label, commit in (('base', base), ('head', head)):
                    snapshot = collector.snapshot(label, commit)
                    for path in paths:
                        with self.subTest(label=label, path=path):
                            self.assertEqual((snapshot / path).read_text(), f'{label} fixture\n')
                    self.assertFalse((snapshot / 'docs/quality').exists())


class RiskScopeTests(unittest.TestCase):
    def test_report_retention_scope_preserves_compiler_configuration_identity(self):
        from source_measure import SERIES, SOURCE_FILES
        self.assertEqual(SERIES, {
            'analyzer': 'harness-gate-rust-measure/0.3.1', 'rule': 'mccabe-rust-3/1',
            'instrumentation': 'closure-black-box/1', 'mapping': 'insertions-utf8/1',
            'selection': 'gh33-findings/1', 'configuration': 'compiler-target-production/4',
        })
        self.assertIn('process/command.rs', SOURCE_FILES)
        self.assertIn('process/task.rs', SOURCE_FILES)
        self.assertIn('process/capture.rs', SOURCE_FILES)
        self.assertIn('config/diagnostic.rs', SOURCE_FILES)
        self.assertIn('error.rs', SOURCE_FILES)
        self.assertIn('config/suggestions.rs', SOURCE_FILES)
        self.assertIn('app/commands.rs', SOURCE_FILES)
        self.assertEqual({path for path in SOURCE_FILES if path.startswith('service/')},
                         {'service/lease.rs', 'service/lease/container.rs', 'service/mod.rs', 'service/report_directory.rs'})

    def test_process_test_module_reaches_measurement_but_unknown_sources_block(self):
        with tempfile.TemporaryDirectory() as temporary:
            collector = gate.Collector(Path(temporary) / 'candidate', 'base', 'head', 'scope-test')
            for path, supported in (
                ('tools/harness-gate/src/process/tests.rs', True),
                ('tools/harness-gate/src/process/replay.rs', True),
                ('tools/harness-gate/src/process/command.rs', True),
                ('tools/harness-gate/src/process/capture.rs', True),
                ('tools/harness-gate/src/project/discovery.rs', True),
                ('tools/harness-gate/src/project/mod.rs', True),
                ('tools/harness-gate/src/utils/redaction.rs', True),
                ('tools/harness-gate/src/scope/detection.rs', True),
                ('tools/harness-gate/src/findings/tests.rs', True),
                ('tools/harness-gate/src/findings/mod.rs', True),
                ('tools/harness-gate/src/findings/unknown.rs', False),
                ('tools/harness-gate/src/scope/unknown.rs', False),
                ('tools/harness-gate/src/utils/unknown.rs', False),
                ('tools/harness-gate/src/process/unknown.rs', False),
                ('tools/harness-gate/src/process/tests/unknown.rs', False),
                ('tools/harness-gate/src/unknown/tests.rs', False),
            ):
                with self.subTest(path=path), \
                        patch.object(gate.subprocess, 'check_output', return_value=path + '\n'), \
                        patch.object(gate, 'relocated_migration_sources', return_value=set()), \
                        patch.object(gate, 'only_terminal_test_module_changed', return_value=False), \
                        patch.object(collector, 'command',
                                     side_effect=RuntimeError('pinned measurer reached')) as command:
                    if supported:
                        with self.assertRaisesRegex(RuntimeError, 'pinned measurer reached'):
                            collector.risk()
                        command.assert_called_once()
                        self.assertEqual(command.call_args.args[0], 'measurer-download')
                    else:
                        with self.assertRaisesRegex(ValueError, 'outside supported risk series'):
                            collector.risk()
                        command.assert_not_called()

    def test_report_retention_sources_reach_measurer_download_but_unknown_service_blocks(self):
        with tempfile.TemporaryDirectory() as temporary:
            collector = gate.Collector(Path(temporary) / 'candidate', 'base', 'head', 'retention-scope')
            for path, supported in (
                ('tools/harness-gate/src/service/lease.rs', True),
                ('tools/harness-gate/src/service/mod.rs', True),
                ('tools/harness-gate/src/service/report_directory.rs', True),
                ('tools/harness-gate/src/service/unknown.rs', False),
            ):
                with self.subTest(path=path), \
                        patch.object(gate.subprocess, 'check_output', return_value=path + '\n'), \
                        patch.object(gate, 'relocated_migration_sources', return_value=set()), \
                        patch.object(gate, 'only_terminal_test_module_changed', return_value=False), \
                        patch.object(collector, 'command',
                                     side_effect=RuntimeError('pinned measurer reached')) as command:
                    if supported:
                        with self.assertRaisesRegex(RuntimeError, 'pinned measurer reached'):
                            collector.risk()
                        command.assert_called_once()
                        self.assertEqual(command.call_args.args[0], 'measurer-download')
                    else:
                        with self.assertRaisesRegex(ValueError, 'outside supported risk series'):
                            collector.risk()
                        command.assert_not_called()


class InlineTestScopeTests(unittest.TestCase):
    SOURCE = 'fn live() { println!("production"); }\n#[cfg(test)]\nmod tests { fn check() {} }\n'

    def prove(self, before, after):
        with patch.object(gate.subprocess, 'check_output',
                          side_effect=[b'100644 blob\0', before, b'100644 blob\0', after]):
            return gate.only_terminal_test_module_changed('base', 'head', 'unknown.rs')

    def test_only_test_body_changes_preserve_exact_production_bytes(self):
        for body in ('fn check() { assert!(true); }',
                     'fn check() { let s = r###"} #[cfg(test)] mod tests {"###; }',
                     '/* outer { /* nested } */ } */ fn check() { let c = b\'}\'; }',
                     'fn check() { let 文本 = "🦀"; }'):
            with self.subTest(body=body):
                changed = self.SOURCE.replace('fn check() {}', body)
                self.assertTrue(self.prove(self.SOURCE.encode(), changed.encode()))

    def test_production_header_suffix_and_encoding_changes_stay_blocked(self):
        variants = {
            'production': self.SOURCE.replace('production', 'changed'),
            'cfg-not': self.SOURCE.replace('cfg(test)', 'cfg(not(test))'),
            'cfg-any': self.SOURCE.replace('cfg(test)', 'cfg(any(test, unix))'),
            'cfg-attr': self.SOURCE.replace('cfg(test)', 'cfg_attr(test, allow(dead_code))'),
            'new-attribute': self.SOURCE.replace('mod tests', '#[allow(dead_code)]\nmod tests'),
            'renamed-module': self.SOURCE.replace('mod tests', 'mod checks'),
            'trailing-production': self.SOURCE + 'fn appended() {}\n',
            'same-line-production': self.SOURCE.rstrip() + ' fn appended() {}\n',
            'trailing-comment': self.SOURCE + '// changed suffix\n',
            'changed-newlines': self.SOURCE.replace('\n', '\r\n'),
            'unicode-production': self.SOURCE.replace('production', '🦀'),
            'unclosed-module': self.SOURCE[:-2],
            'mismatched-delimiter': self.SOURCE.replace('fn check() {}', 'fn check() { (] }'),
            'unclosed-raw-string': self.SOURCE.replace('fn check() {}', 'fn check() { r#"'),
            'unclosed-comment': self.SOURCE + '/*',
        }
        for label, source in variants.items():
            with self.subTest(label=label):
                self.assertFalse(self.prove(self.SOURCE.encode(), source.encode()))
        self.assertFalse(self.prove(self.SOURCE.encode(), self.SOURCE.encode() + b'\xff'))

    def test_spoofed_nested_and_ambiguous_modules_are_not_exempt(self):
        for source in (
            'const S: &str = "#[cfg(test)] mod tests {}";',
            'const S: &str = r###"#[cfg(test)] mod tests {}"###;',
            '// #[cfg(test)] mod tests {}\nfn live() {}',
            '/* #[cfg(test)] mod tests {} */ fn live() {}',
            'macro_rules! fake { () => { #[cfg(test)] mod tests {} }; }',
            'mod outer { #[cfg(test)] mod tests {} }',
            '#[cfg(test)] mod tests {}\n#[cfg(test)] mod tests {}',
            '#[cfg(test)] mod tests {}\nfn live() {}',
        ):
            with self.subTest(source=source):
                self.assertFalse(self.prove(source.encode(), source.encode()))

    def test_shebang_and_bom_cannot_spoof_test_boundaries(self):
        source = ('#! #[cfg(test)] mod tests { /*\nfn main() {\n'
                  '    let _ = "*/"; // "\n    println!("production");\n}\n')
        for prefix in ('', '\ufeff'):
            before = (prefix + source).encode()
            after = (prefix + source.replace('production', 'changed')).encode()
            self.assertFalse(self.prove(before, after))
        self.assertFalse(self.prove(('\ufeff' + self.SOURCE).encode(),
                                    ('\ufeff' + self.SOURCE).encode()))

    def test_missing_sources_and_git_failures_are_not_exempt(self):
        error = subprocess.CalledProcessError(128, ['git', 'show'])
        for results in ([error], [b'100644 blob\0', error],
                        [b'100644 blob\0', self.SOURCE.encode(), error], [OSError('unavailable')]):
            with self.subTest(results=results), \
                    patch.object(gate.subprocess, 'check_output', side_effect=results):
                self.assertFalse(gate.only_terminal_test_module_changed('base', 'head', 'unknown.rs'))

    def test_symlinks_nonregular_files_and_mode_changes_are_not_exempt(self):
        for modes in ((b'120000 blob\0', b'120000 blob\0'),
                      (b'100644 blob\0', b'120000 blob\0'),
                      (b'100644 blob\0', b'100755 blob\0'),
                      (b'040000 tree\0', b'040000 tree\0'),
                      (b'', b''), (b'100644 blob\0' * 2, b'100644 blob\0')):
            with self.subTest(modes=modes), patch.object(
                gate.subprocess, 'check_output',
                side_effect=[modes[0], self.SOURCE.encode(), modes[1], self.SOURCE.encode()]
            ):
                self.assertFalse(gate.only_terminal_test_module_changed('base', 'head', 'unknown.rs'))

    def test_committed_test_only_diff_reaches_collection_but_production_edit_blocks(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / 'tools/harness-gate/src/unknown.rs'
            path.parent.mkdir(parents=True)

            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, stderr=subprocess.PIPE,
                                               text=True).strip()

            git('init')
            git('config', 'user.name', 'Inline Test Scope')
            git('config', 'user.email', 'scope@example.invalid')
            path.write_text(self.SOURCE)
            git('add', '.')
            git('commit', '-m', 'base')
            base = git('rev-parse', 'HEAD')
            for index, (source, allowed) in enumerate((
                (self.SOURCE.replace('fn check() {}', 'fn check() { assert!(true); }'), True),
                (self.SOURCE.replace('production', 'changed'), False),
            )):
                path.write_text(source)
                git('commit', '-am', 'candidate')
                head = git('rev-parse', 'HEAD')
                with patch.object(gate, 'ROOT', root):
                    collector = gate.Collector(root / f'candidate-{index}', base, head, 'test-scope')
                    with patch.object(collector, 'command',
                                      side_effect=RuntimeError('pinned measurer reached')) as command:
                        if allowed:
                            with self.assertRaisesRegex(RuntimeError, 'pinned measurer reached'):
                                collector.risk()
                            command.assert_called_once()
                            self.assertEqual(command.call_args.args[0], 'measurer-download')
                        else:
                            with self.assertRaisesRegex(ValueError, 'outside supported risk series'):
                                collector.risk()
                            command.assert_not_called()

    def test_git_symlink_target_text_cannot_masquerade_as_rust_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = 'tools/harness-gate/src/unknown.rs'

            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, stderr=subprocess.PIPE,
                                               text=True).strip()

            git('init')
            git('config', 'user.name', 'Inline Test Scope')
            git('config', 'user.email', 'scope@example.invalid')
            commits = []
            for name in ('old', 'new'):
                target = f'payload\n#[cfg(test)] mod tests {{{name}}}\n'.encode()
                blob = subprocess.check_output(['git', 'hash-object', '-w', '--stdin'],
                                               input=target, cwd=root).decode().strip()
                # Write the Git symlink entry directly, without depending on
                # the host's symlink privileges or filename restrictions.
                git('update-index', '--add', '--cacheinfo', f'120000,{blob},{path}')
                git('commit', '-m', name)
                commits.append(git('rev-parse', 'HEAD'))
            with patch.object(gate, 'ROOT', root):
                self.assertFalse(gate.only_terminal_test_module_changed(*commits, path))


class AggregateTests(unittest.TestCase):
    def test_every_required_result_fails_closed_on_both_events(self):
        for event in ('push', 'pull_request'):
            needs = {name: {'result': 'success'} for name in gate.COMMON + gate.PUSH_ONLY}
            self.assertEqual(gate.aggregate(event, needs), [])
            required = gate.COMMON + (gate.PUSH_ONLY if event == 'push' else ())
            for name in required:
                for result in ('failure', 'cancelled', 'skipped', '', 'unknown'):
                    with self.subTest(event=event, name=name, result=result):
                        bad = copy.deepcopy(needs)
                        bad[name]['result'] = result
                        self.assertIn(name, gate.aggregate(event, bad))
                missing = copy.deepcopy(needs)
                del missing[name]
                self.assertIn(name, gate.aggregate(event, missing))

    def test_verified_push_reuse_is_narrow_and_still_requires_real_classification(self):
        needs = {name: {'result': 'success'} for name in gate.COMMON + gate.PUSH_ONLY}
        for name in gate.REUSABLE:
            needs[name]['result'] = 'skipped'
        self.assertEqual(gate.aggregate('push', needs, reuse=True), [])
        self.assertEqual(set(gate.aggregate('push', needs)), set(gate.REUSABLE))
        for event, full in [('pull_request', True), ('push', False)]:
            with self.assertRaises(ValueError):
                gate.aggregate(event, needs, full=full, reuse=True)
        for name in gate.COMMON + gate.PUSH_ONLY:
            for state in ['failure', 'cancelled', None, 'unknown']:
                bad = copy.deepcopy(needs)
                bad[name]['result'] = state
                self.assertIn(name, gate.aggregate('push', bad, reuse=True))
            bad = copy.deepcopy(needs)
            del bad[name]
            self.assertIn(name, gate.aggregate('push', bad, reuse=True))
            if name not in gate.REUSABLE:
                bad = copy.deepcopy(needs)
                bad[name]['result'] = 'skipped'
                self.assertIn(name, gate.aggregate('push', bad, reuse=True))

    def test_cli_rejects_negative_fixtures(self):
        for result in ('failure', 'cancelled', 'skipped'):
            needs = {name: {'result': 'success'} for name in gate.COMMON}
            needs['quality-coverage']['result'] = result
            command = [sys.executable, str(Path(gate.__file__)), 'aggregate', '--event',
                       'pull_request', '--needs', json.dumps(needs)]
            self.assertNotEqual(subprocess.run(command, capture_output=True).returncode, 0)

    def test_only_explicit_push_jobs_may_skip_in_pr(self):
        needs = {name: {'result': 'success'} for name in gate.COMMON}
        needs.update({name: {'result': 'skipped'} for name in gate.PUSH_ONLY})
        self.assertEqual(gate.aggregate('pull_request', needs), [])
        self.assertEqual(set(gate.aggregate('push', needs)), set(gate.PUSH_ONLY))
        with self.assertRaises(ValueError):
            gate.aggregate('unknown', needs)

    def test_workflow_keeps_collection_required_and_upload_unconditional(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        shard_bodies = {
            'quality-fast': workflow.split('  quality-fast:\n')[1].split('  quality-risk:\n')[0],
            'quality-risk': workflow.split('  quality-risk:\n')[1].split('  quality-matrix:\n')[0],
            'quality-matrix': workflow.split('  quality-matrix:\n')[1].split('  quality-coverage:\n')[0],
        }
        for name, body in shard_bodies.items():
            with self.subTest(shard=name):
                self.assertNotIn("github.event_name == 'push'", body)
                self.assertIn('fetch-depth: 0', body)
                self.assertIn('ci_quality.py collect', body)
        job = workflow.split('  quality-coverage:\n')[1].split('  quality-generic-shadow:\n')[0]
        self.assertIn('needs: [classify, quality-fast, quality-risk, quality-matrix]', job)
        self.assertIn('ci_quality.py verify', job)
        self.assertIn('fetch-depth: 0', job)
        upload = job.split('- name: Upload quality evidence')[1]
        self.assertIn('if: ${{ always() }}', upload)
        self.assertIn('target/quality/candidate', upload)
        aggregate = workflow.split('  quality-required:\n')[1]
        self.assertIn('if: ${{ always() }}', aggregate)
        self.assertIn('ci_quality.py aggregate', aggregate)
        self.assertIn('NEEDS_JSON: ${{ toJSON(needs) }}', aggregate)
        needs = aggregate.split('needs:')[1].split('runs-on:')[0]
        for name in gate.COMMON + gate.PUSH_ONLY:
            self.assertIn(name, needs)

    def test_coverage_display_name_matches_baseline_and_reuse_consumers(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        job = workflow.split('  quality-coverage:\n')[1].split('  quality-generic-shadow:\n')[0]
        name = re.search(r'^    name: (.+)$', job, re.M)[1]
        import ci_reuse
        self.assertEqual(name, gate.COVERAGE_JOB)
        self.assertIn(name, ci_reuse.REQUIRED_JOBS)


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'raw.json').write_text('{"raw":true}')
        for name in gate.REQUIRED_ARTIFACTS:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('fixture raw evidence')
        self.report = {'schema_version': 1, 'candidate': True, 'commit': 'a' * 40,
                       'base_sha': 'b' * 40, 'run_id': 'fresh-run',
                       'stages': {name: {'status': 'success'} for name in gate.STAGES},
                       'artifacts': {name: sha256(self.root / name)
                                     for name in gate.REQUIRED_ARTIFACTS | {'raw.json'}}}

    def verify(self):
        path = self.root / 'candidate.json'
        path.write_text(json.dumps(self.report))
        return gate.verify(path, 'a' * 40, 'b' * 40, 'fresh-run')

    def test_fresh_candidate_passes(self):
        self.assertTrue(self.verify()['candidate'])

    def test_stale_commit_base_run_or_raw_evidence_rejected(self):
        for key in ('commit', 'base_sha', 'run_id'):
            original = self.report[key]
            self.report[key] = 'stale'
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.verify()
            self.report[key] = original
        (self.root / 'raw.json').write_text('stale profiles')
        with self.assertRaises(ValueError):
            self.verify()
        (self.root / 'raw.json').unlink()
        with self.assertRaises(OSError):
            self.verify()

    def test_partial_failure_cancellation_and_skip_rejected(self):
        for stage in gate.STAGES:
            for status in ('failure', 'cancelled', 'skipped', 'running'):
                self.report['stages'][stage]['status'] = status
                with self.subTest(stage=stage, status=status), self.assertRaises(ValueError):
                    self.verify()
            self.report['stages'][stage]['status'] = 'success'
        del self.report['stages']['risk']
        with self.assertRaises(ValueError):
            self.verify()

    def test_missing_artifacts_and_path_escape_rejected(self):
        original = self.report['artifacts'].copy()
        for name in gate.REQUIRED_ARTIFACTS:
            self.report['artifacts'] = original.copy()
            del self.report['artifacts'][name]
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.verify()
        self.report['artifacts'] = {}
        with self.assertRaises(ValueError):
            self.verify()
        self.report['artifacts'] = original | {'../foreign.json': 'digest'}
        with self.assertRaises(ValueError):
            self.verify()


class MergeTests(unittest.TestCase):
    """Shard merge must reject any evidence substitution before copying."""

    SHARD_FILES = {
        'fast': ['coverage.json', 'coverage.raw.json', 'coverage.lcov',
                 'coverage.cobertura.xml', 'production.json', 'base-source.tar',
                 'base-manifest.json', 'head-source.tar', 'head-manifest.json'],
        'risk': ['base-coverage.json', 'base-coverage.lcov', 'base-coverage.cobertura.xml',
                 'base-risk.json', 'head-coverage.json', 'head-coverage.lcov',
                 'head-coverage.cobertura.xml', 'head-risk.json', 'risk.json'],
        'matrix': ['critical-paths.json', 'critical-path-runs/bundle.json'],
    }

    TOOLS = {tool: f'{tool} version-fixture' for tool in ('rustc', 'cargo', 'nextest', 'llvm-cov')}

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def identity_json(self, digest='0' * 64):
        return json.dumps({'schema': 1, 'tools': self.TOOLS, 'configuration_sha256': digest})

    def shard(self, name, *, stages=None, identity=('b' * 40, 'a' * 40, 'merge-run'),
              tool='valid', files=None, extra=None, tamper=None):
        directory = self.root / f'candidate-{name}'
        directory.mkdir(parents=True)
        files = self.SHARD_FILES[name] if files is None else files
        stages = list(gate.SHARD_STAGES[name]) if stages is None else stages
        artifacts = {}
        for relative in files:
            path = directory / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f'{name} raw evidence')
            artifacts[relative] = sha256(path)
        if tamper:
            (directory / tamper).write_text('tampered bytes')
        base, head, run_id = identity
        report = {'schema_version': 1, 'candidate': True, 'base_sha': base, 'commit': head,
                  'run_id': run_id, 'stages': {stage: {'status': 'success'} for stage in stages},
                  'commands': [{'name': f'{name}-cmd'}], 'artifacts': artifacts}
        if extra:
            report.update(extra)
        (directory / 'candidate.json').write_text(json.dumps(report))
        if tool == 'valid':
            (directory / gate.SHARD_TOOL_IDENTITY).write_text(self.identity_json())
        elif tool != 'missing':
            (directory / gate.SHARD_TOOL_IDENTITY).write_text(tool)
        return directory

    def fresh(self):
        self.temp.cleanup()
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        return self.root

    def merge(self, **overrides):
        shards = {name: overrides.get(name) or self.shard(name) for name in gate.SHARD_STAGES}
        return gate.merge(self.root / 'merged', shards)

    def test_complete_shards_merge_into_a_verified_candidate(self):
        merged = self.merge()
        self.assertEqual(set(merged['stages']), set(gate.STAGES))
        self.assertEqual(set(merged['artifacts']), set(gate.REQUIRED_ARTIFACTS))
        output = self.root / 'merged'
        gate.verify(output / 'candidate.json', 'a' * 40, 'b' * 40, 'merge-run')
        self.assertEqual(json.loads((output / gate.SHARD_TOOL_IDENTITY).read_text()),
                         json.loads(self.identity_json()))

    def test_identity_tool_and_measurer_mismatches_rejected(self):
        factories = {
            'identity': lambda: self.shard('risk', identity=('c' * 40, 'a' * 40, 'merge-run')),
            'tool-differs': lambda: self.shard('matrix', tool=self.identity_json('1' * 64)),
            'tool-missing': lambda: self.shard('fast', tool='missing'),
            'measurer-conflict': lambda: self.shard('fast', extra={'measurer': {'pin': 1}}),
        }
        for label, factory in factories.items():
            with self.subTest(case=label):
                self.fresh()
                overrides = {'risk': factory()} if label == 'identity' else {}
                if label == 'tool-differs':
                    overrides = {'matrix': factory()}
                elif label == 'tool-missing':
                    overrides = {'fast': factory()}
                elif label == 'measurer-conflict':
                    overrides = {'fast': factory(),
                                 'risk': self.shard('risk', extra={'measurer': {'pin': 2}})}
                with self.assertRaises(ValueError):
                    self.merge(**overrides)

    def test_missing_extra_or_failed_stages_rejected(self):
        for name, stages in (('fast', ('legacy',)), ('risk', ()), ('matrix', ('matrix', 'risk'))):
            with self.subTest(shard=name, stages=stages):
                self.fresh()
                with self.assertRaises(ValueError):
                    self.merge(**{name: self.shard(name, stages=stages)})
        # A shard that recorded a failed stage cannot be merged even with its
        # full stage set present.
        self.fresh()
        directory = self.shard('matrix')
        report = json.loads((directory / 'candidate.json').read_text())
        report['stages']['matrix'] = {'status': 'failure', 'error': 'fixture'}
        (directory / 'candidate.json').write_text(json.dumps(report))
        with self.assertRaises(ValueError):
            self.merge(matrix=directory)

    def test_tampered_digest_duplicate_path_and_escape_rejected(self):
        self.fresh()
        tampered = self.shard('risk', tamper='risk.json')
        with self.assertRaises(ValueError):
            self.merge(risk=tampered)

        self.fresh()
        shared = list(self.SHARD_FILES['risk']) + ['coverage.json']
        with self.assertRaises(ValueError):
            self.merge(fast=self.shard('fast'), risk=self.shard('risk', files=shared))

        self.fresh()
        escape = self.shard('matrix', files=[])
        report = json.loads((escape / 'candidate.json').read_text())
        report['artifacts']['../foreign.json'] = 'digest'
        (escape / 'candidate.json').write_text(json.dumps(report))
        with self.assertRaises(ValueError):
            self.merge(matrix=escape)

    def test_partial_shard_set_rejected(self):
        self.fresh()
        with self.assertRaises(ValueError):
            gate.merge(self.root / 'merged', {'fast': self.shard('fast')})

    def test_invalid_tool_identity_content_rejected(self):
        cases = {
            'empty': '',
            'not-json': 'tool-identity-fingerprint',
            'missing-tool': json.dumps({'schema': 1, 'tools': {'rustc': 'v'},
                                        'configuration_sha256': '0' * 64}),
            'empty-value': json.dumps({'schema': 1, 'tools': dict(self.TOOLS, cargo=' '),
                                       'configuration_sha256': '0' * 64}),
            'bad-digest': json.dumps({'schema': 1, 'tools': self.TOOLS,
                                      'configuration_sha256': 'nope'}),
        }
        for label, content in cases.items():
            with self.subTest(case=label):
                self.fresh()
                matrix = self.shard('matrix', files=[], tool=content)
                with self.assertRaises(ValueError):
                    self.merge(matrix=matrix)

    def test_aliased_absolute_dotdot_and_reserved_paths_rejected(self):
        # './coverage.json' aliases 'coverage.json' but must not dodge the
        # duplicate/ownership checks.
        self.fresh()
        with self.assertRaises(ValueError):
            self.merge(fast=self.shard('fast', files=['./coverage.json']))
        for relative in ('/etc/hostname', '../foreign.json', 'candidate.json',
                         gate.SHARD_TOOL_IDENTITY, 'nested//double.json'):
            with self.subTest(relative=relative):
                self.fresh()
                matrix = self.shard('matrix', files=[])
                report = json.loads((matrix / 'candidate.json').read_text())
                report['artifacts'][relative] = 'digest'
                (matrix / 'candidate.json').write_text(json.dumps(report))
                with self.assertRaises(ValueError):
                    self.merge(matrix=matrix)

    def test_existing_output_and_wrong_shard_set_rejected(self):
        fast = self.shard('fast')
        existing = self.root / 'merged'
        existing.mkdir()
        with self.assertRaises(ValueError):
            gate.merge(existing, {'fast': fast})
        with self.assertRaises(ValueError):
            gate.merge(self.root / 'other', {'fast': fast})


class CollectionTests(unittest.TestCase):
    def test_replay_relocation_requires_an_exact_move_in_git(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, text=True,
                                               stderr=subprocess.DEVNULL).strip()

            git('init')
            git('config', 'user.email', 'fixture@example.invalid')
            git('config', 'user.name', 'Fixture')
            old, new = (root / name for name in gate.REPLAY_RELOCATION)
            old.parent.mkdir(parents=True)
            old.write_text('migration-only replay executable\n')
            git('add', '.')
            git('commit', '-m', 'base')
            base = git('rev-parse', 'HEAD')
            new.parent.mkdir(parents=True)
            old.rename(new)
            git('add', '.')
            git('commit', '-m', 'move')
            with patch.object(gate, 'ROOT', root):
                self.assertEqual(gate.relocated_migration_sources(base, 'HEAD'),
                                 {gate.REPLAY_RELOCATION[0]})
                new.write_text('changed executable\n')
                git('add', '.')
                git('commit', '-m', 'change')
                self.assertEqual(gate.relocated_migration_sources(base, 'HEAD'), set())

    def test_failure_retains_evidence_and_runs_remaining_stages(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'fresh'
            with patch.object(gate, 'metadata', return_value={}), \
                    patch.object(gate, 'git_sha', return_value='a' * 40), \
                    patch.object(gate, 'require_committed_sources'):
                collector = gate.Collector(output, 'b' * 40, 'a' * 40, 'test')
                calls = []

                def run(stage):
                    calls.append(stage)
                    (output / f'{stage}.log').write_text('raw stage evidence')
                    if stage == 'legacy':
                        raise ValueError('negative gate fixture')

                for stage in gate.STAGES:
                    setattr(collector, stage, lambda stage=stage: run(stage))
                with self.assertRaises(ValueError):
                    collector.collect()
                self.assertEqual(calls, list(gate.STAGES))
                report = json.loads((output / 'candidate.json').read_text())
                self.assertEqual(report['stages']['legacy']['status'], 'failure')
                self.assertEqual(report['stages']['matrix']['status'], 'success')
                self.assertIn('legacy.log', report['artifacts'])

    def test_existing_candidate_is_never_reused_or_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / 'original'
            marker.write_text('retained')
            with self.assertRaises(ValueError):
                gate.Collector(Path(directory), 'b' * 40, 'a' * 40, 'test')
            self.assertEqual(marker.read_text(), 'retained')

    def test_partial_stage_collection_defers_full_verify_to_the_merge_job(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'fresh'
            with patch.object(gate, 'metadata', return_value={}), \
                    patch.object(gate, 'git_sha', return_value='a' * 40), \
                    patch.object(gate, 'require_committed_sources'), \
                    patch.object(gate, 'verify') as verify_mock:
                collector = gate.Collector(output, 'b' * 40, 'a' * 40, 'test')
                for stage in gate.STAGES:
                    setattr(collector, stage, lambda: None)
                collector.collect(stages=('legacy', 'production'))
                verify_mock.assert_not_called()
                report = json.loads((output / 'candidate.json').read_text())
                self.assertEqual(set(report['stages']), {'legacy', 'production'})
                collector.collect()
                verify_mock.assert_called_once()
                report = json.loads((output / 'candidate.json').read_text())
                self.assertEqual(set(report['stages']), set(gate.STAGES))

    def test_unknown_partial_stage_fails_before_collection(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'fresh'
            with patch.object(gate, 'metadata', return_value={}), \
                    patch.object(gate, 'git_sha', return_value='a' * 40), \
                    patch.object(gate, 'require_committed_sources'):
                collector = gate.Collector(output, 'b' * 40, 'a' * 40, 'test')
                with self.assertRaises(ValueError):
                    collector.collect(stages=('legacy', 'unknown'))

    def test_duplicate_and_dependent_partial_stages_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'fresh'
            with patch.object(gate, 'metadata', return_value={}), \
                    patch.object(gate, 'git_sha', return_value='a' * 40), \
                    patch.object(gate, 'require_committed_sources'):
                collector = gate.Collector(output, 'b' * 40, 'a' * 40, 'test')
                with self.assertRaises(ValueError):
                    collector.collect(stages=('legacy', 'legacy'))
                with self.assertRaises(ValueError):
                    collector.collect(stages=('production',))

    def test_failed_partial_stage_exits_nonzero_for_the_shard_job(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'fresh'
            with patch.object(gate, 'metadata', return_value={}), \
                    patch.object(gate, 'git_sha', return_value='a' * 40), \
                    patch.object(gate, 'require_committed_sources'):
                collector = gate.Collector(output, 'b' * 40, 'a' * 40, 'test')

                def fail():
                    raise ValueError('negative gate fixture')

                collector.matrix = fail
                with self.assertRaises(ValueError):
                    collector.collect(stages=('matrix',))
                report = json.loads((output / 'candidate.json').read_text())
                self.assertEqual(report['stages']['matrix']['status'], 'failure')


class ReviewedRemovalTests(unittest.TestCase):
    """The reviewed deletion is exempt only while the old path is actually gone."""

    PATH = 'tools/harness-gate/src/service/postgres.rs'

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.path = self.root / self.PATH
        self.path.parent.mkdir(parents=True)
        self.git('init')
        self.git('config', 'user.name', 'Reviewed Removal')
        self.git('config', 'user.email', 'removal@example.invalid')

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root,
                                       stderr=subprocess.PIPE, text=True).strip()

    def commit(self, message):
        self.git('add', '-A')
        self.git('commit', '--allow-empty', '-m', message)
        return self.git('rev-parse', 'HEAD')

    def risk(self, base, head, name):
        return gate.Collector(self.root / name, base, head, name)

    def test_deletion_at_head_is_the_only_exemption(self):
        self.path.write_text('fn validate() {}\n')
        base = self.commit('base with the old path')
        self.path.unlink()
        head = self.commit('delete the old path')
        collector = self.risk(base, head, 'confirmed-deletion')
        with patch.object(gate, 'ROOT', self.root), \
                patch.object(collector, 'command',
                             side_effect=RuntimeError('pinned measurer reached')) as command:
            with self.assertRaisesRegex(RuntimeError, 'pinned measurer reached'):
                collector.risk()
        self.assertEqual(command.call_args.args[0], 'measurer-download')

    def test_recreated_path_is_rejected(self):
        (self.root / 'README.md').write_text('base without the old path\n')
        base = self.commit('base without the old path')
        self.path.write_text('fn validate() {}\n')
        head = self.commit('recreate the old path')
        collector = self.risk(base, head, 'recreated-path')
        with patch.object(gate, 'ROOT', self.root), \
                patch.object(collector, 'command',
                             side_effect=RuntimeError('pinned measurer reached')) as command:
            with self.assertRaisesRegex(ValueError, 'outside supported risk series'):
                collector.risk()
        command.assert_not_called()

    def test_modified_path_is_rejected(self):
        self.path.write_text('fn validate() {}\n')
        base = self.commit('base with the old path')
        self.path.write_text('fn validate() { let _ = 1; }\n')
        head = self.commit('modify the old path')
        collector = self.risk(base, head, 'modified-path')
        with patch.object(gate, 'ROOT', self.root), \
                patch.object(collector, 'command',
                             side_effect=RuntimeError('pinned measurer reached')) as command:
            with self.assertRaisesRegex(ValueError, 'outside supported risk series'):
                collector.risk()
        command.assert_not_called()


if __name__ == '__main__':
    unittest.main()
