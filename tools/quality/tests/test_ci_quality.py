import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ci_quality as gate
from quality_common import ROOT, sha256


class SnapshotTests(unittest.TestCase):
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
            'selection': 'gh315-review-low/1', 'configuration': 'compiler-target-production/4',
        })
        self.assertIn('process/command.rs', SOURCE_FILES)
        self.assertNotIn('process/task.rs', SOURCE_FILES)
        self.assertNotIn('process/capture.rs', SOURCE_FILES)
        self.assertEqual({path for path in SOURCE_FILES if path.startswith('service/')},
                         {'service/lease.rs', 'service/mod.rs', 'service/report_directory.rs'})

    def test_process_test_module_reaches_measurement_but_unknown_sources_block(self):
        with tempfile.TemporaryDirectory() as temporary:
            collector = gate.Collector(Path(temporary) / 'candidate', 'base', 'head', 'scope-test')
            for path, supported in (
                ('tools/harness-gate/src/process/tests.rs', True),
                ('tools/harness-gate/src/process/replay.rs', True),
                ('tools/harness-gate/src/process/command.rs', True),
                ('tools/harness-gate/src/project/discovery.rs', True),
                ('tools/harness-gate/src/project/mod.rs', True),
                ('tools/harness-gate/src/utils/redaction.rs', True),
                ('tools/harness-gate/src/scope/detection.rs', True),
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
            gate.aggregate('workflow_dispatch', needs)

    def test_workflow_keeps_collection_required_and_upload_unconditional(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        job = workflow.split('  quality-coverage:\n')[1].split('  quality-contracts:\n')[0]
        self.assertNotIn("github.event_name == 'push'", job)
        self.assertIn('fetch-depth: 0', job)
        self.assertIn('ci_quality.py collect', job)
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
