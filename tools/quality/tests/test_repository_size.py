"""Exercise the incremental guard against real Git history and hostile filenames."""
import contextlib
import io
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
spec = importlib.util.spec_from_file_location(
    'repository_size', Path(__file__).resolve().parents[2] / 'maintenance/repository_size.py')
size = importlib.util.module_from_spec(spec)
spec.loader.exec_module(size)


class RepositorySizeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.git('init', '-q')
        self.git('config', 'user.name', 'Storage tests')
        self.git('config', 'user.email', 'storage@example.invalid')
        self.git('config', 'commit.gpgsign', 'false')
        self.policy = {'schema': 'repository-size-exceptions/v1', 'exceptions': []}
        self.save_policy()
        self.base = self.commit('baseline')

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root,
                                       stderr=subprocess.PIPE).decode().strip()

    def write(self, path, value):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(value if isinstance(value, bytes) else value.encode())

    def save_policy(self):
        self.write(size.POLICY, json.dumps(self.policy))

    def commit(self, message):
        self.git('add', '-A')
        self.git('commit', '-qm', message, '--allow-empty')
        return self.git('rev-parse', 'HEAD')

    def result(self, base=None, head='HEAD'):
        return size.evaluate(self.root, base or self.base, head)

    def allow(self, path, maximum):
        self.policy['exceptions'].append({'path': path, 'owner': 'core-maintainers',
                                         'reason': 'Frozen compatibility input',
                                         'max_bytes': maximum})
        self.save_policy()

    def add_bulk(self):
        # Each file is legal individually, but 11 blobs cross the bulk threshold.
        for index in range(11):
            self.write(f'payload/{index}.bin', bytes([index]) * size.FILE_LIMIT)
        self.commit('bulk data')

    def save_review(self, report):
        receipt = report['review_template']
        receipt['owner'] = 'core-maintainers'
        receipt['reason'] = 'Reviewed compatibility corpus; required by replay'
        self.write(size.REVIEW, json.dumps(receipt))
        return receipt

    def test_unchanged_historical_large_files_and_deletions_do_not_fail(self):
        self.write('legacy.bin', b'x' * (size.FILE_LIMIT + 1))
        base = self.commit('legacy large blob')
        self.write('README.md', 'small change')
        self.commit('documentation')
        report = self.result(base)
        self.assertEqual(report['errors'], [])
        self.assertEqual([entry['path'] for entry in report['blobs']], ['README.md'])
        (self.root / 'legacy.bin').unlink()
        self.commit('remove legacy')
        self.assertEqual(self.result(base)['errors'], [])

    def test_boundary_sizes_and_exact_bounded_exception(self):
        self.write('limit.bin', b'x' * size.FILE_LIMIT)
        self.write('large.bin', b'y' * (size.FILE_LIMIT + 1))
        self.commit('boundary sizes')
        self.assertEqual(len(self.result()['errors']), 1)
        self.allow('large.bin', size.FILE_LIMIT + 1)
        self.commit('review exception')
        self.assertEqual(self.result()['errors'], [])
        self.write('large.bin', b'z' * (size.FILE_LIMIT + 2))
        self.commit('grow past exception')
        self.assertEqual(len(self.result()['errors']), 1)

    def test_existing_large_file_modified_to_another_large_blob_needs_exception(self):
        self.write('old.bin', b'x' * (size.FILE_LIMIT + 1))
        base = self.commit('historical')
        self.write('old.bin', b'y' * (size.FILE_LIMIT + 1))
        self.commit('change historical')
        self.assertEqual(len(self.result(base)['errors']), 1)

    def test_renamed_large_file_cannot_reuse_old_path_exception(self):
        self.write('old.bin', b'x' * (size.FILE_LIMIT + 1))
        self.allow('old.bin', size.FILE_LIMIT + 1)
        base = self.commit('historical exception')
        self.git('mv', 'old.bin', 'new.bin')
        self.commit('rename')
        report = self.result(base)
        self.assertEqual(len(report['errors']), 1)
        self.assertEqual(report['blobs'][0]['path'], 'new.bin')

    def test_mode_only_change_has_no_new_blob(self):
        self.write('existing.bin', b'x' * (size.FILE_LIMIT + 1))
        base = self.commit('historical')
        self.git('update-index', '--chmod=+x', 'existing.bin')
        self.git('commit', '-qm', 'mode only')
        self.assertEqual(self.result(base)['blobs'], [])

    def test_transient_large_blob_is_not_hidden_by_removal(self):
        self.write('temporary.bin', b'x' * (size.FILE_LIMIT + 1))
        self.commit('oversized intermediate')
        (self.root / 'temporary.bin').unlink()
        self.commit('remove before PR review')
        report = self.result()
        self.assertEqual(len(report['errors']), 1)
        self.assertEqual(report['blobs'][0]['path'], 'temporary.bin')

    def test_merged_side_branch_transient_blob_is_checked(self):
        main = self.git('symbolic-ref', '--short', 'HEAD')
        self.git('checkout', '-qb', 'side')
        self.write('side.bin', b'x' * (size.FILE_LIMIT + 1))
        self.commit('large side blob')
        (self.root / 'side.bin').unlink()
        self.commit('remove side blob')
        self.git('checkout', '-q', main)
        self.write('main.txt', 'main change')
        self.commit('main work')
        self.git('merge', '--no-ff', 'side', '-m', 'merge side')
        self.assertEqual(len(self.result()['errors']), 1)

    def test_literal_paths_roundtrip_without_line_splitting_or_globbing(self):
        paths = ['-leading', 'two\nlines\tand spaces', ':(glob)**.bin', 'literal*.bin',
                 '中文名字', 'back\\slash']
        if os.name != 'nt':
            paths.append(os.fsdecode(b'invalid-\xff'))
        for path in paths:
            self.write(path, b'x' * (size.FILE_LIMIT + 1))
            self.allow(path, size.FILE_LIMIT + 1)
        self.commit('odd but valid Git filenames')
        report = self.result()
        self.assertEqual(report['errors'], [])
        self.assertEqual({entry['path'] for entry in report['blobs']} - {size.POLICY}, set(paths))
        self.assertEqual(json.loads(json.dumps(report)), report)

    def test_glob_shaped_exception_matches_only_literal_filename(self):
        self.allow('*.bin', size.FILE_LIMIT + 1)
        self.write('actual.bin', b'x' * (size.FILE_LIMIT + 1))
        self.commit('literal exception')
        self.assertEqual(len(self.result()['errors']), 1)

    def test_exact_bulk_threshold_does_not_need_review(self):
        for index in range(10):
            self.write(f'payload/{index}.bin', bytes([index]) * size.FILE_LIMIT)
        self.commit('exact threshold')
        report = self.result()
        self.assertEqual(report['changed_blob_bytes'], size.CHANGE_LIMIT)
        self.assertEqual(report['errors'], [])

    def test_cli_writes_reviewable_failure_report(self):
        self.write('oversized.bin', b'x' * (size.FILE_LIMIT + 1))
        self.commit('oversized')
        output = self.root / 'report.json'
        with patch.object(size, 'ROOT', self.root), \
                patch.object(sys, 'argv', ['repository_size.py', '--base-sha', self.base,
                                          '--output', str(output)]), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(size.main(), 1)
        report = json.loads(output.read_text())
        self.assertEqual(len(report['errors']), 1)
        self.assertEqual(report['blobs'][0]['path'], 'oversized.bin')

    def test_bulk_review_requires_current_base_and_complete_blob_inventory(self):
        self.add_bulk()
        report = self.result()
        self.assertEqual(len(report['errors']), 1)
        receipt = self.save_review(report)
        self.commit('review corpus')
        self.assertEqual(self.result()['errors'], [])
        receipt['blobs'].pop()
        self.write(size.REVIEW, json.dumps(receipt))
        self.commit('incomplete review')
        self.assertIn('incomplete', self.result()['errors'][0])
        receipt = self.save_review(self.result())
        receipt['base'] = '0' * 40
        self.write(size.REVIEW, json.dumps(receipt))
        self.commit('wrong review base')
        self.assertIn('base does not match', self.result()['errors'][0])

    def test_bulk_review_does_not_exempt_large_individual_file(self):
        self.write('oversized.bin', b'x' * (size.CHANGE_LIMIT + 1))
        self.commit('single oversized blob')
        self.save_review(self.result())
        self.commit('bulk review only')
        report = self.result()
        self.assertEqual(len(report['errors']), 1)
        self.assertIn('exact path exception', report['errors'][0])

    def test_review_survives_squash_while_intermediate_versions_remain_listed(self):
        self.add_bulk()
        self.write('payload/0.bin', b'z' * size.FILE_LIMIT)
        self.commit('intermediate version')
        self.save_review(self.result())
        self.commit('review all versions')
        self.assertEqual(self.result()['errors'], [])
        tree_oid = self.git('rev-parse', 'HEAD^{tree}')
        squash = self.git('commit-tree', tree_oid, '-p', self.base, '-m', 'squash')
        self.assertEqual(self.result(head=squash)['errors'], [])

    def test_repeated_versions_at_one_path_count_once_each(self):
        self.write('changed.txt', 'one')
        self.commit('one')
        self.write('changed.txt', 'two')
        self.commit('two')
        self.write('changed.txt', 'one')
        self.commit('one again')
        report = self.result()
        self.assertEqual(report['changed_blob_bytes'], 6)
        self.assertEqual(len(report['blobs']), 2)

    def test_policy_is_read_from_head_commit_not_local_worktree(self):
        self.write('oversized.bin', b'x' * (size.FILE_LIMIT + 1))
        self.commit('oversized')
        self.allow('oversized.bin', size.FILE_LIMIT + 1)
        self.assertEqual(len(self.result()['errors']), 1)

    def test_missing_zero_and_invalid_commit_fail_closed(self):
        for base in ('', '0' * 40, '0' * 64, '--help', 'missing-revision'):
            with self.subTest(base=base), self.assertRaises((ValueError, subprocess.CalledProcessError)):
                size.evaluate(self.root, base, 'HEAD')

    def test_nonancestor_push_includes_endpoint_changes(self):
        self.write('large.bin', b'x' * (size.FILE_LIMIT + 1))
        earlier = self.commit('large historical file')
        (self.root / 'large.bin').unlink()
        later = self.commit('remove')
        self.assertEqual(len(self.result(later, earlier)['errors']), 1)

    def test_symlink_policy_and_self_exception_are_rejected(self):
        self.allow(size.REVIEW, size.FILE_LIMIT + 1)
        self.commit('self exemption')
        with self.assertRaisesRegex(ValueError, 'cannot exempt itself'):
            self.result()
        (self.root / size.POLICY).unlink()
        (self.root / size.POLICY).symlink_to('other.json')
        self.commit('policy symlink')
        with self.assertRaisesRegex(ValueError, 'regular file'):
            self.result()

    def test_exception_schema_rejects_unbounded_and_ambiguous_entries(self):
        valid = {'path': 'fixture.bin', 'owner': 'team', 'reason': 'needed',
                 'max_bytes': size.FILE_LIMIT + 1}
        for key, value in (('path', '../escape'), ('path', '/absolute'), ('owner', ' '),
                           ('reason', ''), ('max_bytes', True), ('max_bytes', -1)):
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                size.exceptions({'schema': 'repository-size-exceptions/v1',
                                 'exceptions': [valid | {key: value}]})
        with self.assertRaisesRegex(ValueError, 'duplicate exception'):
            size.exceptions({'schema': 'repository-size-exceptions/v1',
                             'exceptions': [valid, valid]})
        with self.assertRaisesRegex(ValueError, 'duplicate JSON field'):
            json.loads('{"owner":"a","owner":"b"}', object_pairs_hook=size.unique_fields)

    def test_missing_policy_fails_instead_of_disabling_check(self):
        (self.root / size.POLICY).unlink()
        self.commit('remove policy')
        with self.assertRaisesRegex(ValueError, 'missing committed policy'):
            self.result()

    def test_ci_runs_guard_before_classification_on_required_path(self):
        workflow = (size.ROOT / '.github/workflows/ci.yml').read_text()
        classify = workflow.split('  classify:\n', 1)[1].split('\n  test:', 1)[0]
        self.assertIn('fetch-depth: 0', classify)
        self.assertIn('set -euo pipefail', classify)
        self.assertLess(classify.index('python3 tools/maintenance/repository_size.py'),
                        classify.index('python3 tools/quality/ci_quality.py classify'))
        self.assertNotIn('continue-on-error', classify)
        self.assertNotRegex(classify, re.compile(r'^\s+if:', re.M))
        import ci_quality
        failures = ci_quality.aggregate('pull_request', {'classify': {'result': 'failure'}}, False)
        self.assertIn('classify', failures)


if __name__ == '__main__':
    unittest.main()
