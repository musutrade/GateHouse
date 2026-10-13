"""Prepare unsigned inventory without asserting successful release eligibility."""
import argparse
from pathlib import Path
import tempfile

import collector_assets as assets
import install_collector as installer


def verify_payloads(directory, manifest):
    # Reuse the installer's complete archive/type/path/hash checks without
    # executing payloads or pretending this unsigned candidate is installable.
    with tempfile.TemporaryDirectory(prefix='collector-candidate-', dir=directory.parent) as tmp:
        installer.extract(directory / 'collector.tar', Path(tmp), manifest)


def prepare(directory):
    assets.require(set(p.name for p in directory.iterdir()) == {'collector.tar', 'manifest.json'},
                   'fresh candidate directory must contain only archive and manifest')
    manifest = assets.contract.load_manifest((directory / 'manifest.json').read_bytes())
    tag = 'rust-collector-v' + assets.version(manifest['collector']['version'])
    verify_payloads(directory, manifest)
    assets.write(directory / 'sbom.spdx.json', assets.sbom(manifest))
    # This is deliberately a preparation receipt, never production provenance.
    receipt = {'schema': 'rust-collector-preparation/v1', 'tag': tag,
        'source_commit': manifest['source_commit'], 'status': 'unsigned-ineligible',
        'assets': [dict(row, size=(directory / row['name']).stat().st_size)
                   for row in assets.subjects(directory, assets.ASSETS[:3])],
        'missing': ['production provenance', 'protected eligibility', 'RSA signature',
                    'Sigstore bundle', 'reviewed final compatibility', 'license approval']}
    return receipt


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--receipt', type=Path, required=True)
    args = parser.parse_args()
    assets.write(args.receipt, prepare(args.directory))
