"""Inventory retained preparation bytes; never produce redistribution approval."""
import hashlib
import json
from pathlib import Path
import sys
import tarfile

base = Path(sys.argv[1])
output = Path(sys.argv[2])
manifest_bytes = (base / 'manifest.json').read_bytes()
manifest = json.loads(manifest_bytes)
rows = []
with tarfile.open(base / 'collector.tar') as archive:
    entries = archive.getmembers()
    members = {member.name: member for member in entries}
    if len(members) != len(entries):
        raise ValueError('duplicate archive member')
    paths = [payload['path'] for payload in manifest['payloads']]
    if len(set(paths)) != len(paths):
        raise ValueError('duplicate manifest payload')
    for payload in manifest['payloads']:
        member = members[payload['path']]
        if not member.isfile():
            raise ValueError('payload is not a regular file: ' + member.name)
        with archive.extractfile(member) as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        if digest != payload['sha256']:
            raise ValueError('payload digest mismatch: ' + member.name)
        rows.append(dict(path=member.name, sha256=digest, size=member.size,
                         license='NOASSERTION', redistribution='pending',
                         license_evidence=[], obligations_evidence=[]))
result = dict(schema='rust-collector-license-worklist/v1', status='pending',
              scope='historical GH-231 collector only; bootstrap and final RC excluded',
              reviewer=None, proposed_reviewer='higoalespn',
              source_commit=manifest['source_commit'],
              manifest_sha256=hashlib.sha256(manifest_bytes).hexdigest(),
              payloads=rows)
output.write_text(json.dumps(result, indent=2) + '\n')
print('Verified historical payload hashes:', len(rows), '; approvals: 0')
