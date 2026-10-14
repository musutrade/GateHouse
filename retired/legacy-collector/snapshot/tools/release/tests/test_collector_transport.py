import copy
import io
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import subprocess
import base64

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collector_assets as assets
import collector_transport as transport
import friendly_collector_install as installer


class TransportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def archive(self, name, plugin=b'first plugin'):
        path = self.root / name
        with tarfile.open(path, 'w', format=tarfile.PAX_FORMAT) as archive:
            directory = tarfile.TarInfo('rust'); directory.type = tarfile.DIRTYPE
            archive.addfile(directory)
            for name, data in [('rust/LLVM', b'shared tool' * 1000),
                               ('rust/lib/LLVM', b'shared tool' * 1000),
                               ('bin/collector', plugin)]:
                info = tarfile.TarInfo(name); info.size = len(data); info.mode = 0o755
                info.pax_headers = {'mtime': '123.456'}
                archive.addfile(info, io.BytesIO(data))
        return path

    def test_exact_roundtrip_dedup_and_plugin_upgrade_reuses_toolchain(self):
        first = self.archive('first.tar')
        a = transport.pack(first, self.root/'a')
        b = transport.pack(self.archive('next.tar', b'changed plugin'), self.root/'b')
        self.assertEqual(a['bundles'][0], b['bundles'][0])
        self.assertEqual(len(a['bundles'][0]['objects']), 1)
        self.assertNotEqual(a['bundles'][1]['sha256'], b['bundles'][1]['sha256'])
        for name, descriptor in [('a',a),('b',b)]:
            target = self.root/(name+'.tar')
            transport.reconstruct(descriptor, self.root/name, target, self.root/'objects')
            self.assertEqual(assets.sha(target), descriptor['archive_sha256'])
        self.assertEqual(first.read_bytes(), (self.root/'a.tar').read_bytes())

    def test_corrupt_bundle_and_cache_are_rejected(self):
        d = transport.pack(self.archive('x.tar'), self.root/'bundle')
        p = self.root/'bundle'/d['bundles'][0]['name']
        original = p.read_bytes(); p.write_bytes(original+b'bad')
        with self.assertRaisesRegex(ValueError, 'bundle checksum'):
            transport.reconstruct(d,self.root/'bundle',self.root/'result',self.root/'cache')
        p.write_bytes(original)
        transport.reconstruct(d,self.root/'bundle',self.root/'result',self.root/'cache')
        digest = next(iter(d['bundles'][0]['objects']))
        (self.root/'cache'/digest).write_bytes(b'corrupted')
        with self.assertRaisesRegex(ValueError, 'corrupt cached'):
            transport.reconstruct(d,self.root/'bundle',self.root/'other',self.root/'cache')

    def test_recipe_overflow_and_path_injection_rejected(self):
        d = transport.pack(self.archive('x.tar'), self.root/'bundle')
        bad=copy.deepcopy(d);bad['bundles'][0]['objects']['../escape']=10
        with self.assertRaisesRegex(ValueError,'invalid blob'):
            transport.reconstruct(bad,self.root/'bundle',self.root/'bad',self.root/'cache')
        bad=copy.deepcopy(d);bad['archive_size']=1
        with self.assertRaises(ValueError):
            transport.reconstruct(bad,self.root/'bundle',self.root/'bad',self.root/'cache')

    def test_offline_download_cache_survives_removed_source(self):
        source=self.root/'offline';source.mkdir();cache=self.root/'cache';cache.mkdir()
        p=source/'tools.gz';p.write_bytes(b'locked tools')
        row={'name':p.name,'size':p.stat().st_size,'sha256':assets.sha(p)}
        a=installer.fetch(row,'https://example.invalid',cache,source);p.unlink()
        self.assertEqual(a,installer.fetch(row,'https://example.invalid',cache,source))
        a.write_bytes(b'bad')
        with self.assertRaisesRegex(ValueError,'corrupt download cache'):
            installer.fetch(row,'https://example.invalid',cache,source)

    def test_download_retries_short_ranges_and_reuses_completed_ranges(self):
        data=b'release bytes'; calls=[]
        def curl(args,**kwargs):
            calls.append(args)
            output=Path(args[args.index('--output')+1])
            headers=Path(args[args.index('--dump-header')+1])
            output.write_bytes(data[:3] if len(calls)==1 else data)
            headers.write_text(f'HTTP/1.1 206 Partial Content\nContent-Range: bytes 0-{len(data)-1}/{len(data)}\n')
            return subprocess.CompletedProcess(args,18 if len(calls)==1 else 0)
        target=self.root/'download.part'
        with patch.object(installer.subprocess,'run',side_effect=curl):
            installer.download_parts('https://example.invalid/file',target,len(data))
        self.assertEqual(len(calls),2)
        self.assertEqual(target.read_bytes(),data)
        self.assertIn('--http1.1',calls[0])
        with patch.object(installer.subprocess,'run',side_effect=AssertionError('unexpected network')):
            installer.download_parts('https://example.invalid/file',target,len(data))

    def test_server_ignoring_range_is_rejected(self):
        def curl(args,**kwargs):
            Path(args[args.index('--output')+1]).write_bytes(b'payload')
            Path(args[args.index('--dump-header')+1]).write_text('HTTP/1.1 200 OK\n')
            return subprocess.CompletedProcess(args,0)
        with patch.object(installer.subprocess,'run',side_effect=curl):
            with self.assertRaisesRegex(ValueError,'Download interrupted'):
                installer.download_parts('https://example.invalid/file',self.root/'bad.part',7)

    def test_pinned_existing_verifier_is_reused_but_changed_binary_is_not(self):
        verifier=self.root/'cosign';verifier.write_bytes(b'independently pinned verifier fixture')
        openssl=self.root/'openssl';openssl.write_bytes(b'host verifier fixture')
        row={'name':'cosign-linux-amd64','size':verifier.stat().st_size,'sha256':assets.sha(verifier)}
        host={'schema':'rust-collector-host-trust/v2','openssl':str(openssl),'openssl_sha256':assets.sha(openssl)}
        profile={'host':host,'verifier':row}
        for key in ('public_key','trusted_root'):
            p=self.root/key;p.write_bytes(key.encode())
            host[key+'_sha256']=assets.sha(p);profile[key]=base64.b64encode(p.read_bytes()).decode()
        catalog={'trust':profile,'host_abi':{},'base_url':'https://example.invalid'}
        offline=self.root/'offline';offline.mkdir()
        cache=self.root/'cache';cache.mkdir()
        with patch.object(installer.assets,'probe_host',return_value={}), patch.object(installer.shutil,'which',return_value=str(verifier)):
            trust=installer.provision(catalog,cache/'trust',cache,offline)
            self.assertEqual(Path(trust['cosign']).read_bytes(),verifier.read_bytes())
            verifier.write_bytes(b'untrusted replacement')
            other=self.root/'other';other.mkdir()
            with self.assertRaises(FileNotFoundError):
                installer.provision(catalog,other/'trust',other,offline)


if __name__=='__main__': unittest.main()
