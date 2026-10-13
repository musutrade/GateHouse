"""Local RSA-signed synthetic lifecycle coverage; not clean-host certification."""
import os
import multiprocessing
from pathlib import Path
import unittest
from unittest.mock import patch

import test_collector_delivery as fixtures
import tarfile
import io
import subprocess
import shutil
from contextlib import redirect_stdout
from types import SimpleNamespace
import build_user_installer as builder
import collector_assets as assets
import collector_components as components
import collector_maintenance as maintenance
import collector_receipt as receipt
import collector_store as store
import collector_light_install as light
import install_collector as lifecycle
import friendly_collector_install as friendly


def clean_in_process(root, trust, ready, done):
    ready.set()
    with lifecycle.locked(root):
        maintenance.cleanup_locked(root, trust, keep=0)
    done.set()


class ComponentTests(unittest.TestCase):
    setUpClass = classmethod(fixtures.DeliveryTests.setUpClass.__func__)
    tearDownClass = classmethod(fixtures.DeliveryTests.tearDownClass.__func__)
    setUp = fixtures.DeliveryTests.setUp
    make_archive = fixtures.DeliveryTests.make_archive
    sign = fixtures.DeliveryTests.sign
    resign_inventory = fixtures.DeliveryTests.resign_inventory

    def prepare(self, version=1, *, tool=False):
        self.manifest['collector']['version'] = '0.1.0-rc.' + str(version)
        self.tag = 'rust-collector-v' + self.manifest['collector']['version']
        name = self.manifest['tools'][0]['path'] if tool else 'bin/harness-gate-rust-collector'
        self.payload[name] += str(version).encode()
        for row in self.manifest['payloads'] + self.manifest['tools']:
            row['sha256'] = assets.hashlib.sha256(self.payload[row['path']]).hexdigest()
        self.make_archive()
        self.sign()
        return components.pack(self.release / 'collector.tar', self.base / ('objects-' + str(version)))

    def install_components(self, descriptor, **kwargs):
        cache = self.base / 'cache'
        cache.mkdir(mode=0o700, exist_ok=True)
        with lifecycle.locked(self.root):
            plan, sources = components.plan(descriptor, self.root, cache, mode='pinned')
            for row in plan['objects']:
                destination = cache / row['object']
                if not destination.exists():
                    components.expand(self.base / ('objects-' + self.tag.rsplit('.', 1)[1]) / row['name'],
                                      destination, {'size': row['expanded_size'], 'sha256': row['object']})
                sources[row['object']] = destination
            result = components.install(self.root, self.release, descriptor, sources, self.trust, self.tag, **kwargs)
        return result, plan

    def test_three_plugin_upgrades_tool_upgrade_retention_and_rollback(self):
        first, _ = self.install_components(self.prepare())
        tool_path = self.manifest['tools'][0]['path']
        inode = (first.parent.parent / tool_path).stat().st_ino
        for version in (2, 3, 4):
            installed, plan = self.install_components(self.prepare(version))
            self.assertEqual(len(plan['objects']), 1)
            self.assertEqual((installed.parent.parent / tool_path).stat().st_ino, inode)
            self.assertFalse((installed.parent.parent / '.delivery/collector.tar').exists())
            lifecycle.select(self.root, '0.1.0-rc.' + str(version), self.trust)
        installed, plan = self.install_components(self.prepare(5, tool=True))
        self.assertEqual(len(plan['objects']), 1)
        self.assertNotEqual((installed.parent.parent / tool_path).stat().st_ino, inode)
        lifecycle.select(self.root, '0.1.0-rc.4', self.trust)
        with lifecycle.locked(self.root):
            result = maintenance.cleanup_locked(self.root, self.trust, keep=2)
        self.assertIn('0.1.0-rc.4', result['retained'])
        self.assertEqual(len(result['retained']), 3)
        self.assertGreater(result['reclaimed_allocated_bytes'], 0)
        lifecycle.select(self.root, '0.1.0-rc.5', self.trust)

    def test_plugin_revision_does_not_change_toolchain_revision(self):
        descriptor = self.prepare()
        tool = dict(descriptor['payloads'][0], path='rust/bin/rustc')
        descriptor['payloads'].append(tool)
        before = components.component_versions(descriptor)
        descriptor = self.prepare(2)
        descriptor['payloads'].append(tool)
        after = components.component_versions(descriptor)
        self.assertNotEqual(before['plugin']['revision'], after['plugin']['revision'])
        self.assertEqual(before['rust-llvm'], after['rust-llvm'])

    def test_cleanup_waits_for_install_lock_and_preserves_references(self):
        self.install_components(self.prepare())
        context = multiprocessing.get_context('spawn')
        ready = context.Event()
        done = context.Event()
        with lifecycle.locked(self.root):
            child = context.Process(target=clean_in_process, args=(self.root, self.trust, ready, done))
            child.start()
            self.assertTrue(ready.wait(5))
            self.assertFalse(done.wait(0.1))
        child.join(10)
        if child.is_alive():
            child.terminate()
            child.join()
            self.fail('cleanup did not finish')
        self.assertEqual(child.exitcode, 0)
        lifecycle.select(self.root, '0.1.0-rc.1', self.trust)

    def test_exact_external_reuse_is_private_copy_and_version_similarity_insufficient(self):
        descriptor = self.prepare()
        external = self.base / 'external'
        external.mkdir()
        lifecycle.extract(self.release / 'collector.tar', external, self.manifest)
        with lifecycle.locked(self.root):
            plan, sources = components.plan(descriptor, self.root, self.base / 'cache', reuse=external)
            self.assertEqual(plan['download_bytes'], 0)
            components.install(self.root, self.release, descriptor, sources, self.trust, self.tag)
        installed = self.root / 'versions/0.1.0-rc.1'
        path = self.manifest['tools'][0]['path']
        self.assertNotEqual((external / path).stat().st_ino, (installed / path).stat().st_ino)
        (external / path).write_bytes(b'same version, different build')
        lifecycle.select(self.root, '0.1.0-rc.1', self.trust)
        with lifecycle.locked(self.base / 'other'):
            plan, _ = components.plan(descriptor, self.base / 'other', self.base / 'cache', reuse=external)
        # All synthetic tool paths share identical bytes; change every candidate.
        for row in self.manifest['tools']:
            (external / row['path']).write_bytes(b'incompatible')
        with lifecycle.locked(self.base / 'other'):
            plan, _ = components.plan(descriptor, self.base / 'other', self.base / 'cache', reuse=external)
        self.assertGreater(plan['download_bytes'], 0)

    def test_legacy_migration_export_and_interrupted_receipt_write(self):
        with patch.object(lifecycle, 'compact'):
            lifecycle.install(self.root, self.release, self.trust, self.tag)
        original = assets.sha(self.release / 'collector.tar')
        version = self.root / 'versions/0.1.0-rc.1'
        real_replace = os.replace
        def fail_receipt(source, target):
            if Path(target).name == receipt.NAME:
                raise OSError('simulated migration interruption')
            return real_replace(source, target)
        with patch.object(lifecycle.os, 'replace', side_effect=fail_receipt):
            with self.assertRaisesRegex(OSError, 'interruption'):
                maintenance.migrate(self.root, self.trust)
        lifecycle.select(self.root, '0.1.0-rc.1', self.trust)
        report = maintenance.migrate(self.root, self.trust)
        self.assertGreater(report['reclaimed_allocated_bytes'], 0)
        self.assertFalse((version / '.delivery/collector.tar').exists())
        output = self.base / 'export'
        maintenance.export(self.root, '0.1.0-rc.1', self.trust, output)
        self.assertEqual(assets.sha(output / 'collector.tar'), original)
        assets.verify(output, self.trust, self.tag)

    def test_failed_selfcheck_and_disk_write_preserve_current_and_recover(self):
        self.install_components(self.prepare())
        descriptor = self.prepare(2)
        def fail(_):
            raise ValueError('self-check failed')
        with self.assertRaisesRegex(ValueError, 'self-check'):
            self.install_components(descriptor, self_check=fail)
        self.assertEqual(assets.read(self.root / 'current.json')['version'], '0.1.0-rc.1')
        self.assertFalse((self.root / 'versions/0.1.0-rc.2').exists())
        with lifecycle.locked(self.root):
            self.assertEqual(list((self.root / 'staging').iterdir()), [])
            store.collect(self.root)
        lifecycle.select(self.root, '0.1.0-rc.1', self.trust)
        with patch.object(components.shutil, 'copyfile', side_effect=OSError(28, 'No space left on device')):
            with self.assertRaises(OSError):
                self.install_components(descriptor)
        with lifecycle.locked(self.root):
            self.assertEqual(list((self.root / 'staging').iterdir()), [])
        self.install_components(descriptor)

    def test_receipt_and_shared_content_tampering_block_selection(self):
        self.install_components(self.prepare())
        root = self.root / 'versions/0.1.0-rc.1'
        path = root / '.delivery/archive-receipt.json'
        original = path.read_bytes()
        value = assets.read(path)
        value['rows'][0]['prefix'] = value['rows'][1]['prefix']
        assets.write(path, value)
        with self.assertRaises((ValueError, tarfile.ReadError)):
            lifecycle.select(self.root, '0.1.0-rc.1', self.trust)
        path.write_bytes(original)
        tool = root / self.manifest['tools'][0]['path']
        tool.chmod(0o644)
        with self.assertRaisesRegex(ValueError, 'mode'):
            lifecycle.select(self.root, '0.1.0-rc.1', self.trust)
        tool.chmod(0o755)
        tool.write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            lifecycle.select(self.root, '0.1.0-rc.1', self.trust)

    def test_cleanup_refuses_unowned_content_and_store_symlinks(self):
        self.install_components(self.prepare())
        extra = self.root / 'versions/0.1.0-rc.1/project.txt'
        extra.write_text('must survive')
        with lifecycle.locked(self.root), self.assertRaisesRegex(ValueError, 'extra'):
            maintenance.cleanup_locked(self.root, self.trust, keep=0)
        self.assertTrue(extra.exists())
        extra.unlink()
        target = self.root / 'objects/unknown'
        target.symlink_to(self.base / 'external')
        with lifecycle.locked(self.root), self.assertRaises(ValueError):
            store.collect(self.root)
        self.assertTrue(target.is_symlink())

    def test_corrupt_and_oversize_compressed_object_cleans_partial(self):
        descriptor = self.prepare()
        digest, row = next(iter(descriptor['objects'].items()))
        destination = self.base / 'expanded'
        with self.assertRaisesRegex(ValueError, 'exceeds'):
            components.expand(self.base / 'objects-1' / row['name'], destination,
                              {'size': row['expanded_size'] - 1, 'sha256': digest})
        self.assertFalse(destination.exists())
        with self.assertRaisesRegex(ValueError, 'checksum'):
            components.expand(self.base / 'objects-1' / row['name'], destination,
                              {'size': row['expanded_size'], 'sha256': '0' * 64})
        self.assertFalse(destination.exists())

    def test_cache_capacity_does_not_remove_unknown_files(self):
        cache = self.base / 'cache'
        cache.mkdir()
        unrelated = cache / 'project.txt'
        unrelated.write_bytes(b'project')
        content = b'cache'
        known = cache / (assets.hashlib.sha256(content).hexdigest() + '-object.gz')
        known.write_bytes(content)
        self.assertEqual(light.prune_cache(cache, 0), len(content))
        self.assertTrue(unrelated.exists())
        self.assertFalse(known.exists())

    def test_preflight_policy_and_space_fail_before_acquisition(self):
        descriptor = self.prepare()
        catalog = {'components': descriptor, 'archive_sha256': descriptor['archive_sha256'],
                   'host_abi': self.manifest['host_abi'], 'compatibility': {}, 'controls': [],
                   'trust': {'host': self.trust, 'verifier':
                             {'name': 'cosign', 'sha256': '0' * 64, 'size': 10}}}
        kwargs = dict(mode='pinned', policy='deny', reuse=None, plan_only=False, cache_limit=0)
        with patch('friendly_collector_install.fetch') as fetch, patch('friendly_collector_install.provision') as provision, redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(ValueError, 'download denied'):
                light.install(catalog, self.root, self.base / 'cache', None, **kwargs)
            kwargs['policy'] = 'allow'
            with patch.object(light.shutil, 'disk_usage', return_value=SimpleNamespace(free=0)):
                with self.assertRaisesRegex(ValueError, 'insufficient disk'):
                    light.install(catalog, self.root, self.base / 'cache', None, **kwargs)
            fetch.assert_not_called()
            provision.assert_not_called()
        self.assertFalse((self.root / 'current.json').exists())

    def test_offline_user_entry_authenticates_and_installs_missing_objects(self):
        descriptor = self.prepare()
        kit = self.base / 'objects-1'
        controls = []
        for name in assets.ASSETS[1:] + assets.CONTROL:
            path = self.release / name
            shutil.copyfile(path, kit / name)
            controls.append({'name': name, 'size': path.stat().st_size, 'sha256': assets.sha(path)})
        catalog = {'version': self.manifest['collector']['version'], 'components': descriptor,
                   'archive_sha256': descriptor['archive_sha256'], 'host_abi': self.manifest['host_abi'],
                   'compatibility': {key: self.manifest[key] for key in ('tools', 'measurement', 'core_compatibility')},
                   'controls': controls, 'base_url': 'https://invalid.example', 'release_url': 'https://invalid.example',
                   'trust': {'host': self.trust, 'verifier': {'name': 'cosign', 'sha256': '0' * 64, 'size': 10}}}
        # Test-only local RSA authority and synthetic executables. Acquisition,
        # signed replay, compatibility, sharing and activation are the real path.
        catalog['object_base_urls'] = builder.object_locations(descriptor, catalog['base_url'])
        with patch('friendly_collector_install.provision', return_value=self.trust), patch.object(light, 'self_check') as check, patch.object(friendly, 'fetch', wraps=friendly.fetch) as fetch, redirect_stdout(io.StringIO()):
            launcher = light.install(catalog, self.root, self.base / 'cache', kit,
                                    mode='pinned', policy='deny', reuse=None, plan_only=False, cache_limit=0)
        check.assert_called_once()
        for call in fetch.call_args_list:
            row, base_url = call.args[:2]
            if 'object' in row:
                self.assertEqual(base_url, catalog['object_base_urls'][row['object']])
        self.assertTrue(launcher.is_file())
        lifecycle.select(self.root, self.manifest['collector']['version'], self.trust)
        self.assertFalse((launcher.parent.parent / '.delivery/collector.tar').exists())

    def test_bootstrap_uncached_plan_and_default_deny_do_not_download(self):
        script = self.base / 'install.sh'
        script.write_text(builder.TEMPLATE.replace('@BOOTSTRAP_PLAN@', '{"bootstrap_bytes":123}'))
        cache = self.base / 'cache'
        for args, code in ((['--plan'], 0), ([], 1)):
            result = subprocess.run(['bash', str(script), '--cache-dir', str(cache), *args],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, code, result.stderr)
            self.assertIn('"bootstrap_bytes":123', result.stdout)
            self.assertEqual({p.name for p in cache.iterdir()}, {'lock'})

    def test_cache_usage_counts_resumable_ranges_and_preserves_them(self):
        cache = self.base / 'cache'
        cache.mkdir()
        ranges = cache / ('a' * 64 + '-object.gz.part-ranges')
        ranges.mkdir()
        (ranges / '0').write_bytes(b'partial')
        (ranges / '0.headers').write_bytes(b'header')
        self.assertEqual(light.cache_usage(cache)['partial_bytes'], 13)
        self.assertEqual(light.prune_cache(cache, 0), 0)
        self.assertTrue((ranges / '0').exists())


if __name__ == '__main__':
    unittest.main()
