"""Archive/projection paths without a retained corpus dependency."""
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

import rust_reference as reference


class SourceRootsTests(unittest.TestCase):
    def archive(self, root, rows):
        path = root / 'sources.tar'
        with tarfile.open(path, 'w') as bundle:
            for name, kind in rows:
                info = tarfile.TarInfo(name)
                info.type = kind
                if kind == tarfile.REGTYPE:
                    payload = b'fn measured() {}\n'
                    info.size = len(payload)
                    bundle.addfile(info, io.BytesIO(payload))
                else:
                    info.linkname = '/outside'
                    bundle.addfile(info)
        return path

    def test_projection_and_extraction_agree_for_all_allowed_roots(self):
        sources = ['process/adapter.rs', '../quality-core/policy.rs',
                   '../runtime-protocol/src/lib.rs', '../runtime-protocol/src/wire.rs']
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rows = [(reference.source_path(source), tarfile.REGTYPE) for source in sources]
            rows.append(('tools/harness-gate/other-crate/src/lib.rs', tarfile.REGTYPE))
            reference.sources_from_archive(self.archive(root, rows), root / 'out')
            for source in sources:
                self.assertEqual((root / 'out' / reference.source_path(source)).read_bytes(), b'fn measured() {}\n')
            self.assertFalse((root / 'out/tools/harness-gate/other-crate').exists())

    def test_unreviewed_siblings_and_noncanonical_sources_reject(self):
        for source in ['../other/src/lib.rs', '../runtime-protocol-other/src/lib.rs',
                       '../runtime-protocol/src/../Cargo.toml', '../runtime-protocol/src/./lib.rs',
                       '../runtime-protocol/src//lib.rs', '../../escape.rs']:
            with self.subTest(source=source), self.assertRaises(ValueError):
                reference.source_path(source)

    def test_selected_archive_rejects_aliases_duplicates_and_links(self):
        base = 'tools/harness-gate/runtime-protocol/src/'
        cases = [([(base + 'lib.rs', tarfile.REGTYPE)] * 2),
                 [(base + '../escape.rs', tarfile.REGTYPE)],
                 [(base + './lib.rs', tarfile.REGTYPE)],
                 [(base + '/lib.rs', tarfile.REGTYPE)],
                 [(base + 'lib.rs', tarfile.SYMTYPE)],
                 [(base + 'lib.rs', tarfile.LNKTYPE)]]
        for rows in cases:
            with self.subTest(rows=rows), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                with self.assertRaises(ValueError):
                    reference.sources_from_archive(self.archive(root, rows), root / 'out')
