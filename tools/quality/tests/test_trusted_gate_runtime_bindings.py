"""Runtime input bindings cannot become an alternate quality authority."""
import copy
import importlib.util
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest
import uuid
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('bound_trusted_gate',
    ROOT / 'tools/trusted_gate/__init__.py', submodule_search_locations=[str(ROOT / 'tools/trusted_gate')])
PACKAGE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = PACKAGE
SPEC.loader.exec_module(PACKAGE)
from bound_trusted_gate import runtime as RUNTIME, isolation as ISOLATION, execution as EXECUTION
from bound_trusted_gate.cache import DIMENSIONS, dimensions, identity
from bound_trusted_gate.common import Blocked, canonical, digest, protected
from bound_trusted_gate.control import Control


class RuntimeBindingTests(unittest.TestCase):
    def fixture(self):
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        self.addCleanup(temporary.cleanup)
        def writable():
            for parent, _, names in os.walk(root):
                os.chmod(parent, 0o700)
                for name in names:
                    path = Path(parent) / name
                    if path.is_file() and not path.is_symlink():
                        path.chmod(0o600)
        self.addCleanup(writable)
        tools = root / 'tools'
        (tools / 'bin').mkdir(parents=True)
        files = {}
        for name in ('rustc', 'cargo', 'cargo-nextest', 'node', 'rustfmt', 'cargo-clippy'):
            content = ('reviewed ' + name).encode()
            path = tools / 'bin' / name
            path.write_bytes(content)
            path.chmod(0o555)
            files['bin/' + name] = [len(content), digest(content), 0o555]
        (tools / 'bin').chmod(0o555)
        tools.chmod(0o555)
        config = {'runtime': {'root': str(tools), 'inventory': str(root / 'inventory.json'),
                             'tool_lock_sha256': 'b' * 64}, 'protected_files': {}}
        value = {'schema': 'gatehouse-official-tools/v1', 'tool_lock_sha256': 'b' * 64,
                 'scope': RUNTIME.SCOPE, 'files': files, 'directories': ['bin'],
                 'bytes': sum(row[0] for row in files.values()), 'archive_bytes': 1}
        self.repin(config, value)
        return root, config, value

    def repin(self, config, value):
        path = Path(config['runtime']['inventory'])
        path.write_bytes(canonical(value) + b'\n')
        config['protected_files'][str(path)] = digest(path.read_bytes())

    def verify(self, config):
        # Only Root ownership is replaced for ordinary-user fixture trees.
        # The streaming reads, no-links, modes, inventory and hashes are real.
        with patch.object(RUNTIME, 'protected', side_effect=lambda path, **kw: Path(path)):
            return RUNTIME.verify(config)

    def test_exact_runtime_bytes_modes_and_inventory_are_verified_without_execution(self):
        _, config, value = self.fixture()
        with patch('subprocess.run', side_effect=AssertionError('must not execute runtime')):
            report = self.verify(config)
        self.assertEqual(report['files'], 6)
        self.assertEqual(report['bytes'], value['bytes'])
        self.assertEqual(report['inventory_sha256'], config['protected_files'][config['runtime']['inventory']])
        self.assertIsNone(RUNTIME.verify({}))

    def test_runtime_requires_real_protected_ownership_outside_fixture_patch(self):
        _, config, _ = self.fixture()
        # Simulate the ordinary developer owner even on a Root-run test host.
        original = Path.lstat
        def developer_stat(path):
            item = original(path)
            values = list(item)
            values[4] = 1000
            return os.stat_result(values)
        with patch.object(Path, 'lstat', developer_stat), self.assertRaisesRegex(Blocked, 'root-owned'):
            RUNTIME.verify(config)

    def test_large_payload_has_separate_bound_and_generic_reader_is_unchanged(self):
        root, config, value = self.fixture()
        path = root / 'tools/bin/node'
        path.chmod(0o600)
        size = 128 * 1024 * 1024 + 1
        with path.open('wb') as stream:
            stream.truncate(size)
        import hashlib
        sha = hashlib.sha256()
        with path.open('rb') as source:
            while block := source.read(1024 * 1024):
                sha.update(block)
        path.chmod(0o555)
        old_size = value['files']['bin/node'][0]
        value['files']['bin/node'] = [size, sha.hexdigest(), 0o555]
        value['bytes'] += size - old_size
        self.repin(config, value)
        self.assertEqual(self.verify(config)['bytes'], value['bytes'])
        with self.assertRaisesRegex(Blocked, 'bounded regular'):
            RUNTIME.read_regular(path, 128 * 1024 * 1024)
        with patch.object(RUNTIME, 'MAX_FILE', size - 1), self.assertRaises(Blocked):
            self.verify(config)

    def test_unpinned_or_forged_inventory_cannot_authorize_changed_binary(self):
        root, config, value = self.fixture()
        binary = root / 'tools/bin/node'
        binary.chmod(0o755)
        binary.write_bytes(b'reviewed fake')
        binary.chmod(0o555)
        with self.assertRaisesRegex(Blocked, 'size|bytes'):
            self.verify(config)
        value['files']['bin/node'] = [binary.stat().st_size, digest(binary.read_bytes()), 0o555]
        value['bytes'] = sum(row[0] for row in value['files'].values())
        Path(config['runtime']['inventory']).write_bytes(canonical(value) + b'\n')
        with self.assertRaisesRegex(Blocked, 'pin differs'):
            self.verify(config)
        config['protected_files'].clear()
        with self.assertRaisesRegex(Blocked, 'not pinned'):
            self.verify(config)

    def test_missing_extra_links_hardlinks_fifos_and_mutable_modes_are_rejected(self):
        for attack in ('missing', 'extra-file', 'extra-dir', 'leaf-link', 'directory-link',
                       'hardlink', 'fifo', 'writable-file', 'writable-dir', 'privileged-mode'):
            root, config, _ = self.fixture()
            tools, binary = root / 'tools', root / 'tools/bin/node'
            tools.chmod(0o755)
            (tools / 'bin').chmod(0o755)
            if attack == 'missing':
                binary.unlink()
            elif attack == 'extra-file':
                (tools / 'unexpected').write_bytes(b'extra')
            elif attack == 'extra-dir':
                (tools / 'unexpected').mkdir()
            elif attack == 'leaf-link':
                binary.unlink()
                binary.symlink_to(tools / 'bin/cargo')
            elif attack == 'directory-link':
                (tools / 'bin').rename(root / 'outside')
                (tools / 'bin').symlink_to(root / 'outside', target_is_directory=True)
            elif attack == 'hardlink':
                os.link(binary, root / 'outside-link')
            elif attack == 'fifo':
                binary.unlink()
                os.mkfifo(binary)
            elif attack in ('writable-file', 'privileged-mode'):
                binary.chmod(0o755 if attack == 'writable-file' else 0o4555)
            if attack != 'directory-link':
                (tools / 'bin').chmod(0o555)
            tools.chmod(0o755 if attack == 'writable-dir' else 0o555)
            with self.subTest(attack=attack), self.assertRaises(Blocked):
                self.verify(config)

    def test_malformed_repinning_is_rejected_before_payload_access(self):
        changes = [lambda v: v.update(schema='unknown'), lambda v: v.update(bytes=True),
                   lambda v: v.update(bytes=v['bytes'] + 1), lambda v: v.update(archive_bytes=0),
                   lambda v: v.update(directories=['bin', 'bin']),
                   lambda v: v.update(directories=['bin', 'absent']),
                   lambda v: v['files'].update({'../escape': [0, 'a' * 64, 0o444]}),
                   lambda v: v['files'].update({'/absolute': [0, 'a' * 64, 0o444]}),
                   lambda v: v['files'].update({'bin//alias': [0, 'a' * 64, 0o444]}),
                   lambda v: v['files'].update({'bin/node': [True, 'a' * 64, 0o555]}),
                   lambda v: v['files'].update({'bin/node': [1, 'bad hash', 0o555]}),
                   lambda v: v['files'].update({'bin/node': [1, 'a' * 64, 0o755]}),
                   lambda v: v['files'].update({'bin/node': [RUNTIME.MAX_FILE + 1, 'a' * 64, 0o555]}),
                   lambda v: v.update(tool_lock_sha256='c' * 64), lambda v: v.update(unreviewed=True)]
        for index, change in enumerate(changes):
            _, config, value = self.fixture()
            change(value)
            self.repin(config, value)
            with self.subTest(index=index), self.assertRaises(Blocked):
                self.verify(config)
        _, config, _ = self.fixture()
        for field, limit in (('MAX_TOTAL', 1), ('MAX_MEMBERS', 1), ('MAX_INVENTORY', 1)):
            with patch.object(RUNTIME, field, limit), self.subTest(field=field), self.assertRaises(Blocked):
                self.verify(config)

    def test_same_bytes_rewritten_during_streaming_are_not_accepted(self):
        root, config, _ = self.fixture()
        binary = root / 'tools/bin/node'
        original = os.fdopen
        class ChangingFile:
            def __init__(self, fd, mode):
                self.stream = original(fd, mode)
                self.changed = False
            def __enter__(self):
                return self
            def __exit__(self, *args):
                self.stream.close()
            def fileno(self):
                return self.stream.fileno()
            def read(self, limit):
                data = self.stream.read(limit)
                if not self.changed and os.fstat(self.fileno()).st_ino == binary.stat().st_ino:
                    self.changed = True
                    binary.chmod(0o755)
                    binary.write_bytes(data)
                    binary.chmod(0o555)
                return data
        with patch.object(RUNTIME.os, 'fdopen', side_effect=ChangingFile), self.assertRaises(Blocked):
            self.verify(config)

    def test_ambiguous_unprotected_and_mutable_mount_operands_are_rejected(self):
        root, config, _ = self.fixture()
        for value in ('relative', '/a/../b', '/a//b', '/a/./b', '/a b', '/a:b', '/a%h', '/a\\x', '/a\n'):
            changed = copy.deepcopy(config)
            changed['runtime']['root'] = value
            with self.subTest(value=value), self.assertRaisesRegex(Blocked, 'mount operands'):
                self.verify(changed)
        for value in (None, {}, {**config['runtime'], 'extra': True}):
            with self.subTest(value=value), self.assertRaises(Blocked):
                self.verify({**config, 'runtime': value})
        for field in ('source', 'scratch', 'inbox'):
            with self.subTest(field=field), self.assertRaisesRegex(Blocked, 'overlap'):
                self.verify({**config, field: str(root)})

    def test_readonly_runtime_and_cargo_bindings_keep_path_and_environment_guards(self):
        root, config, _ = self.fixture()
        cargo = root / 'cargo-config.toml'
        cargo.write_text('[net]\noffline = true\n')
        config.update(cargo_config=str(cargo), source='/source', records='/control',
                      executor_user='worker', executor_memory_bytes=1024**3, executor_file_bytes=1024**3,
                      steps={'tests': {'argv': ['/usr/bin/python3', '-B', 'job.py'], 'timeout_seconds': 60}})
        config['protected_files'].update({str(cargo): digest(cargo.read_bytes()), '/usr/bin/python3': 'pin'})
        self.verify(config)
        with patch.object(RUNTIME, 'protected', side_effect=lambda path, **kw: Path(path)):
            args = ISOLATION.sandbox_arguments(config, {'id': 'run'}, 'tests', '/scratch', '/cache')
        readonly = next(arg for arg in args if arg.startswith('--property=BindReadOnlyPaths='))
        self.assertIn(str(root / 'tools') + ':/opt/gatehouse/toolchain', readonly)
        self.assertIn(str(cargo) + ':/work/output/cargo-home/config.toml', readonly)
        self.assertIn('--setenv=PATH=/opt/gatehouse/toolchain/bin:/usr/local/bin:/usr/bin:/bin', args)
        for name in ('PATH', 'CARGO_HOME', 'LD_LIBRARY_PATH', 'GIT_CONFIG_GLOBAL'):
            config['steps']['tests']['env'] = {name: '/untrusted'}
            with patch.object(RUNTIME, 'protected', side_effect=lambda path, **kw: Path(path)), \
                 self.subTest(name=name), self.assertRaises(Blocked):
                ISOLATION.sandbox_arguments(config, {'id': 'run'}, 'tests', '/scratch', '/cache')
        for name in ('cargo_registry', 'cargo_git'):
            with self.subTest(name=name), self.assertRaisesRegex(Blocked, 'cannot mix'):
                self.verify({**config, name: '/raw'})
        cargo.write_text('changed')
        with self.assertRaisesRegex(Blocked, 'Cargo configuration differs'):
            self.verify(config)

    def test_runtime_mutation_invalidates_the_current_policy_before_publication(self):
        root, config, _ = self.fixture()
        core = root / 'core'
        core.write_bytes(b'Core fixture')
        for path in [core, *list((ROOT / 'tools/trusted_gate').glob('*.py'))]:
            config['protected_files'][str(path)] = digest(path.read_bytes())
        executor = EXECUTION.Executor.__new__(EXECUTION.Executor)
        executor.config, executor.core, executor.configuration_path = config, core, None
        with patch.object(RUNTIME, 'protected', side_effect=lambda path, **kw: Path(path)), \
             patch.object(EXECUTION, 'protected', side_effect=lambda path, **kw: Path(path)):
            pin = executor.policy_identity()
            from types import SimpleNamespace
            run = {'id': 'run', 'repository': 'owner/repo', 'head': 'a' * 40, 'input': {'policy': pin}}
            journal = SimpleNamespace(latest=lambda *args: run)
            control = Control(journal, None, executor, config)
            control.current(run)
            binary = root / 'tools/bin/node'
            binary.chmod(0o755)
            binary.write_bytes(b'changed')
            binary.chmod(0o555)
            with self.assertRaises(Blocked):
                control.current(run)


class JournalArgumentTests(unittest.TestCase):
    def journal_run(self):
        return {'id': str(uuid.uuid4()), 'head': 'a' * 40,
                'input': {'head': 'a' * 40, 'base': 'b' * 40}}

    def step(self):
        return {'argv': ['/usr/bin/python3', '-B', 'job.py', '--head', '@gatehouse:head@',
                         '--base', '@gatehouse:base@', '--identity', '@gatehouse:receipt@'],
                'run_arguments': True}

    def test_only_canonical_journal_facts_replace_complete_tokens(self):
        run, step = self.journal_run(), self.step()
        before = copy.deepcopy(step)
        args = ISOLATION.run_arguments({}, run, step)
        self.assertEqual(args[4:], [run['head'], '--base', run['input']['base'],
                                    '--identity', str(uuid.UUID(run['id']).int)])
        self.assertEqual(step, before)
        self.assertGreater(int(args[-1]), 0)
        self.assertEqual(ISOLATION.run_arguments({}, {}, {'argv': ['/usr/bin/true']}), ['/usr/bin/true'])

    def test_unknown_embedded_tokens_false_optin_and_mixed_identity_fail_closed(self):
        run = self.journal_run()
        for token in ('@gatehouse:unknown@', '--head=@gatehouse:head@', '@gatehouse:source@'):
            step = self.step()
            step['argv'].append(token)
            with self.subTest(token=token), self.assertRaises(Blocked):
                ISOLATION.run_arguments({}, run, step)
        for enabled in (False, 'true', 1):
            with self.subTest(enabled=enabled), self.assertRaises(Blocked):
                ISOLATION.run_arguments({}, run, {**self.step(), 'run_arguments': enabled})
        for change in ({'id': '../path'}, {'id': str(uuid.uuid1())}, {'id': run['id'].upper()},
                       {'head': 'c' * 40}, {'input': {'head': 'a' * 40, 'base': 'main'}}):
            with self.subTest(change=change), self.assertRaises(Blocked):
                ISOLATION.run_arguments({}, {**run, **change}, self.step())

    def test_manifest_digest_can_only_read_the_fixed_frozen_source_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'test-data').mkdir()
            manifest = root / 'test-data/manifest.json'
            manifest.write_bytes(b'exact frozen manifest')
            step = self.step()
            step['argv'].append('@gatehouse:test-data-sha256@')
            with patch.object(ISOLATION, 'protected', side_effect=lambda path, **kw: Path(path)):
                args = ISOLATION.run_arguments({'source': directory}, self.journal_run(), step)
                self.assertEqual(args[-1], digest(manifest.read_bytes()))
                manifest.unlink()
                manifest.symlink_to(root / 'missing')
                with self.assertRaises(Blocked):
                    ISOLATION.run_arguments({'source': directory}, self.journal_run(), step)

    def test_cache_compatibility_uses_template_before_rendering_and_tracks_real_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tool = root / 'compiler'
            tool.write_bytes(b'compiler')
            (root / 'Cargo.lock').write_bytes(b'locked deps')
            (root / 'config.toml').write_bytes(b'build config')
            step = {**self.step(), 'cache_identity': {name: 'declared' for name in DIMENSIONS},
                    'toolchain_files': [str(tool)], 'cache_inputs':
                    {'dependencies': ['Cargo.lock'], 'configuration': ['config.toml']}}
            step['cache_identity']['release_incremental'] = 'not-applicable'
            def key():
                with patch('bound_trusted_gate.cache.protected', side_effect=lambda path, **kw: Path(path)):
                    return identity(dimensions(root, step, 'owner/repo', 'worker:policy'))
            before = copy.deepcopy(step)
            first = key()
            one, two = self.journal_run(), self.journal_run()
            two['head'] = two['input']['head'] = 'c' * 40
            self.assertNotEqual(ISOLATION.run_arguments({}, one, step), ISOLATION.run_arguments({}, two, step))
            self.assertEqual(step, before)
            self.assertEqual(first, key())
            for name in ('compiler', 'Cargo.lock', 'config.toml'):
                (root / name).write_bytes(b'changed ' + name.encode())
                changed = key()
                self.assertNotEqual(first, changed)
                first = changed
            step['argv'].append('--reviewed-profile-change')
            self.assertNotEqual(first, key())

    def test_launch_checks_runtime_then_selects_cache_before_rendering_journal_arguments(self):
        from types import SimpleNamespace
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ('scratch', 'cache', 'records'):
                (root / name).mkdir()
            run, step = self.journal_run(), self.step()
            (root / 'records' / run['id']).mkdir()
            run.update(local='running')
            run['input']['policy'] = 'policy'
            step.update(timeout_seconds=60, toolchain_files=[])
            config = {'steps': {'linux-test': step}, 'executor_user': 'worker', 'developer_uid': os.getuid()+1,
                      'scratch': str(root / 'scratch'), 'records': str(root / 'records'), 'source': '/source',
                      'repository': 'owner/repo', 'protected_files': {'/usr/bin/python3': 'pin'},
                      'executor_memory_bytes': 1024**3, 'executor_file_bytes': 1024**3,
                      'cache': {'root': str(root / 'cache'), 'max_bytes': 1, 'max_entries': 1, 'ttl_seconds': 1}}
            account = SimpleNamespace(pw_uid=os.getuid(), pw_gid=os.getgid(), pw_shell='/usr/sbin/nologin')
            journal = SimpleNamespace(get=lambda run_id: run)
            cache = SimpleNamespace(directory=lambda selected: root / 'cache', maintain=lambda **kw: {})
            order = []
            def select(*args):
                order.append('cache')
                self.assertIn('@gatehouse:head@', args[1]['argv'])
                return {}
            render = ISOLATION.run_arguments
            def rendered(*args):
                order.append('render')
                return render(*args)
            with patch.object(ISOLATION.os, 'geteuid', return_value=0), \
                 patch.object(ISOLATION.pwd, 'getpwnam', return_value=account), \
                 patch.object(ISOLATION, 'unprivileged_identity', return_value=account), \
                 patch.object(ISOLATION.os, 'chown'), \
                 patch.object(ISOLATION, 'protected', side_effect=lambda path, **kw: Path(path)), \
                 patch.object(ISOLATION, 'verify_runtime', side_effect=lambda cfg: order.append('runtime')), \
                 patch.object(ISOLATION, 'Cache', return_value=cache), \
                 patch.object(ISOLATION, 'dimensions', side_effect=select), \
                 patch.object(ISOLATION, 'run_arguments', side_effect=rendered), \
                 patch.object(ISOLATION.subprocess, 'run', return_value=SimpleNamespace(returncode=0)) as process, \
                 patch.object(ISOLATION, 'confirm_units_stopped'):
                self.assertEqual(ISOLATION.launch(config, journal, run['id'], 'linux-test'), 0)
            self.assertEqual(order, ['runtime', 'cache', 'render'])
            actual = process.call_args.args[0]
            self.assertIn(run['head'], actual)
            self.assertNotIn('@gatehouse:head@', actual)


if __name__ == '__main__':
    unittest.main()
