"""Production dual verification; all verifier paths and roots are host inputs.

Keep six release assets: release-inventory.sig is a v2 envelope containing an
RSA signature and a Sigstore bundle, both over the exact inventory bytes.
Legacy RSA-only fixtures remain readable by the v1 development installer.
"""
import base64
from pathlib import Path
import subprocess
import tempfile

import collector_assets as assets

IDENTITY = 'https://github.com/' + assets.REPOSITORY + '/' + assets.WORKFLOW + '@refs/heads/main'
ISSUER = 'https://token.actions.githubusercontent.com'
EXTRA = {'cosign', 'cosign_sha256', 'trusted_root', 'trusted_root_sha256'}
BASE = {'schema', 'openssl', 'openssl_sha256', 'public_key', 'public_key_sha256',
        'rsa_signature_bytes', 'host_libraries'}


def verify(directory, trust):
    assets.require(set(trust) == BASE | EXTRA and
                   trust['schema'] == 'rust-collector-host-trust/v2', 'production host trust v2 required')
    for field in ('cosign', 'trusted_root', 'openssl', 'public_key'):
        path = Path(trust[field])
        assets.require(path.is_absolute(), 'trust paths must be absolute')
        assets.regular(path)
        assets.require(not path.resolve().is_relative_to(directory.resolve()),
                       'release cannot supply host trust')
        assets.require(assets.sha(path) == trust[field + '_sha256'], 'host trust pin changed: ' + field)
    envelope = assets.read(directory / 'release-inventory.sig')
    assets.require(set(envelope) == {'schema', 'rsa_signature', 'sigstore_bundle'} and
                   envelope['schema'] == 'rust-collector-signatures/v2', 'dual signatures required')
    signature = base64.b64decode(envelope['rsa_signature'], validate=True)
    assets.require(isinstance(envelope['sigstore_bundle'], dict) and envelope['sigstore_bundle'],
                   'missing Sigstore bundle')
    with tempfile.TemporaryDirectory(prefix='collector-verify-') as tmp:
        stage = Path(tmp)
        (stage / 'release-inventory.json').write_bytes((directory / 'release-inventory.json').read_bytes())
        (stage / 'release-inventory.sig').write_bytes(signature)
        legacy = {k: v for k, v in trust.items() if k in BASE}
        legacy['schema'] = 'rust-collector-host-trust/v1'
        assets.verify_signature(stage, legacy)
        bundle = stage / 'bundle.json'
        assets.write(bundle, envelope['sigstore_bundle'])
        # No ambient Sigstore overrides, online trust retrieval, regexp identity,
        # insecure tlog flags, or package-selected executables.
        result = subprocess.run([trust['cosign'], 'verify-blob',
            '--bundle', str(bundle), '--trusted-root', trust['trusted_root'], '--offline',
            '--certificate-identity', IDENTITY, '--certificate-oidc-issuer', ISSUER,
            str(stage / 'release-inventory.json')], capture_output=True, timeout=60,
            env={'PATH': '/usr/bin:/bin', 'HOME': str(stage), 'LANG': 'C'})
        assets.require(result.returncode == 0, 'Sigstore identity/inclusion/signature verification failed')


def envelope(rsa_signature, bundle):
    return {'schema': 'rust-collector-signatures/v2',
            'rsa_signature': base64.b64encode(rsa_signature).decode('ascii'),
            'sigstore_bundle': bundle}
