#!/usr/bin/env python3
"""Nonpublishing synthetic release rehearsal with a disposable local signing key.

Never creates a tag, calls GitHub, or uses a production key. The signed eligibility
receipt is a test fixture, not proof that production release approval occurred.
"""
import argparse
from pathlib import Path
import shutil
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / 'tests'))
from test_collector_delivery import DeliveryTests
import collector_assets as assets
import install_collector as installer


def run(output):
    output.mkdir(parents=True, exist_ok=False)
    DeliveryTests.setUpClass()
    case = DeliveryTests()
    try:
        case.setUp()
        shutil.copytree(case.release, output / 'release')
        case.release = output / 'release'
        case.root = output / 'installation'
        shutil.copyfile(case.keydir / 'public.pem', output / 'test-public.pem')
        case.trust = dict(case.trust, public_key=str((output / 'test-public.pem').resolve()))
        assets.write(output / 'test-host-trust.json', case.trust)
        launcher = case.install()
        assets.verify(case.release, case.trust, case.tag)
        installer.select(case.root, case.manifest['collector']['version'], case.trust)
        installer.uninstall(case.root, case.manifest['collector']['version'], case.trust)
        report = {'schema': 'rust-collector-dry-run/v1', 'status': 'pass',
                  'synthetic_payload': True, 'production_eligibility': 'not_evaluated',
                  'protected_approval': 'not_requested', 'publication': 'not_attempted',
                  'tag_created': False, 'native_measurement': 'not_performed',
                  'checks': ['RSA signature', 'exact inventory', 'SPDX subjects', 'provenance subjects',
                             'host ABI probe', 'staged install', 'version selection', 'owned uninstall'],
                  'release_inventory_sha256': assets.sha(case.release / assets.CONTROL[0]),
                  'removed_launcher': str(launcher)}
        assets.write(output / 'report.json', report)
        return report
    finally:
        case.doCleanups()
        DeliveryTests.tearDownClass()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    print(run(parser.parse_args().output.resolve()))
