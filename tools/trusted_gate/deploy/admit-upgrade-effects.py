"""Read-only upgrade effect observation; no revocation, copy, install or activation."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
import time
import urllib.parse
import urllib.request

PILOT = Path('/srv/gatehouse/pilot')
MAX_SECONDS = 120


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def same(B, actual, expected, message):
    B.need(B.canonical(actual) == B.canonical(expected), message)


def transport(B, client, api_proxy):
    """Caller must bind this explicit route to its authenticated Root recipe.

    Retain the accepted client's redirect/TLS handlers. Its empty proxy handler
    ignores environment settings; only an explicit HTTPS proxy is added here.
    """
    if api_proxy is not None:
        B.need(type(api_proxy) is str and len(api_proxy) <= 256, 'explicit API proxy differs')
        value = urllib.parse.urlsplit(api_proxy)
        B.need(value.scheme in ('http', 'https') and value.hostname
               and re.fullmatch('[A-Za-z0-9.-]+', value.hostname) and value.port is not None
               and 1 <= value.port <= 65535 and value.username is None and value.password is None
               and not value.path and not value.query and not value.fragment
               and api_proxy == value.scheme + '://' + value.hostname + ':' + str(value.port),
               'explicit API proxy requires a canonical credential-free host and port')
        class ExplicitHTTPSProxy(urllib.request.ProxyHandler):
            def proxy_open(self,request,proxy,kind):
                B.need(kind=='https' and request.type=='https' and request.host=='api.github.com',
                       'explicit proxy is limited to the GitHub HTTPS API')
                route=urllib.parse.urlsplit(proxy)
                request.set_proxy(route.netloc,route.scheme)
                return None
        client.opener.add_handler(ExplicitHTTPSProxy({'https': api_proxy}))
    return digest(B.canonical({'https_proxy': api_proxy}))


def remote(B, client, config, run, contract, preflight):
    """Independently read the repository, baseline, candidate and exact old Check."""
    repository = client.repo('GET', '')
    branch = client.repo('GET', '/git/ref/heads/main')
    commit = client.repo('GET', '/git/commits/' + run['head'])
    B.need(type(repository['id']) is int and repository['id'] == preflight.REPOSITORY_ID
           and repository['full_name'] == preflight.REPOSITORY and repository['default_branch'] == 'main'
           and branch['ref'] == 'refs/heads/main' and branch['object']['type'] == 'commit'
           and branch['object']['sha'] == contract.BASE and commit['sha'] == run['head']
           and commit['tree']['sha'] == run['input']['tree'], 'upgrade remote repository/base/commit differs')
    check = client.find_check(run, config['check_name'])
    if check is None:
        B.need(run['check_id'] is None, 'persisted upgrade Check is absent remotely')
        observed = None
    else:
        B.need(type(check['id']) is int and check['id'] > 0 and check['id'] != preflight.NEGATIVE_CHECK
               and (run['check_id'] is None or type(run['check_id']) is int and check['id'] == run['check_id'])
               and check['name'] == config['check_name'] and type(check['app']['id']) is int
               and check['app']['id'] == config['app_id'] and check['head_sha'] == run['head']
               and check['external_id'] == run['id'] and check['status'] == 'completed'
               and check['conclusion'] == 'failure', 'upgrade Check revocation is not confirmed remotely')
        observed = {key: check[key] for key in ('id', 'name', 'head_sha', 'external_id', 'status', 'conclusion')}
        observed['app_id'] = check['app']['id']
    return {'repository_id': repository['id'], 'base': contract.BASE,
            'head': run['head'], 'tree': run['input']['tree'], 'check': observed}


def publication(B, keep, saved, preservation, config, run):
    """Verify byte preservation only; never infer quality acceptance from a copy."""
    destination = PILOT / 'control' / keep.RUN / 'publication'
    evidence = json.loads(run['evidence'], object_pairs_hook=B.unique) if run['evidence'] is not None else None
    B.need(saved['evidence_registered'] is (evidence is not None), 'retained evidence registration differs')
    if evidence is None:
        B.need(not destination.exists() and not destination.is_symlink(), 'unregistered publication requires reconciliation')
        return {'registered': False, 'files': 0, 'bytes': 0, 'inventory_sha256': None}
    B.need(isinstance(evidence, dict) and evidence['run'] == keep.RUN, 'registered evidence belongs to another run')
    source = B.path_operand(evidence['directory'])
    B.need(source.parent == Path(config['source']) / '.gatehouse-host/reports/invocations',
           'registered evidence is outside the old candidate')
    prefix = 'source/' + str(source.relative_to(Path(config['source'])))
    original = saved['inventory']
    files = {key[len(prefix)+1:]: row for key, row in original['files'].items() if key.startswith(prefix + '/')}
    directories = {key[len(prefix)+1:]: mode for key, mode in original['directories'].items()
                   if key.startswith(prefix + '/')}
    B.need(prefix in original['directories'] and files and
           {'test_result.json', 'manifest.json', 'artifact-registry.json', 'invocation.json'} <= files.keys(),
           'registered invocation was not completely preserved')
    for name, key in (('test_result.json', 'machine_sha256'), ('manifest.json', 'manifest_sha256'),
                      ('artifact-registry.json', 'registry_sha256')):
        B.need(files[name][1] == evidence[key], 'registered evidence digest differs from preservation')
    if evidence.get('configuration_sha256'):
        key = 'control/' + keep.RUN + '/configuration.json'
        B.need(key in original['files'] and original['files'][key][1] == evidence['configuration_sha256'],
               'registered configuration was not preserved')
        raw = B.read(preservation / 'tree' / key, 1024**2)
        value = json.loads(raw, object_pairs_hook=B.unique)
        B.need(digest(raw) == evidence['configuration_sha256']
               and digest(B.canonical(value)[:-1]) == json.loads(run['input'], object_pairs_hook=B.unique)['policy'],
               'registered configuration differs from the old policy')
    root = B.owned(destination, directory=True, private=True)
    B.need(stat.S_IMODE(root.stat().st_mode) == original['directories'][prefix], 'publication root mode differs')
    seen_files, seen_directories, total, entries = set(), set(), 0, 0
    for parent, names, leaves in os.walk(root):
        parent = Path(parent);B.owned(parent, directory=True)
        relative = str(parent.relative_to(root))
        if parent != root:
            B.need(relative in directories and stat.S_IMODE(parent.stat().st_mode) == directories[relative],
                   'unknown or changed publication directory')
            seen_directories.add(relative)
        entries += 1 + len(leaves)
        B.need(entries <= keep.MAX_ENTRIES, 'publication entry budget exceeded')
        for name in names:B.owned(parent / name, directory=True)
        for name in leaves:
            path = B.owned(parent / name);key = str(path.relative_to(root))
            B.need(key in files, 'unregistered publication file')
            info = path.stat();total += info.st_size
            B.need(info.st_size <= keep.MAX_FILE and total <= keep.MAX_TOTAL, 'publication byte budget exceeded')
            raw = B.read(path, keep.MAX_FILE)
            B.need([len(raw), digest(raw), stat.S_IMODE(info.st_mode)] == files[key]
                   and keep.file_identity(path.stat()) == keep.file_identity(info), 'publication file differs')
            seen_files.add(key)
    B.need(seen_files == files.keys() and seen_directories == directories.keys(), 'partial retained publication')
    return {'registered': True, 'files': len(files), 'bytes': total,
            'inventory_sha256': digest(B.canonical({'files': files, 'directories': directories}))}


def observe(B, helpers, manifest, manifest_pin, preservation, preservation_pin, lock_fd, *, api_proxy=None):
    """Post-revocation prerequisite under the original lease, never full admission.

    The later coordinator must also bind preparation/passive inspection, current
    storage, entry generation and drain before mutation. This function performs
    only API reads with a least-privilege token, and reads protected local data.
    An optional proxy must come from the authenticated Root coordinator recipe,
    whose receipt must bind the returned transport digest; never from Agent env.
    """
    B.need(os.geteuid() == 0, 'administrator authentication required for effect observation')
    B.need(os.uname().sysname == 'Linux' and os.uname().machine == 'x86_64', 'unsupported effect observation platform')
    B.need(type(lock_fd) is int and lock_fd >= 0, 'effect observation requires an inherited controller lease')
    started = time.monotonic();code = B.authenticate(helpers, manifest, manifest_pin)
    helpers, manifest, preservation = map(B.path_operand, (helpers, manifest, preservation))
    keep = B.load(helpers, 'preserve-pilot-state.py');journal = B.load(helpers, 'admit-upgrade-journal.py')
    fence = B.load(helpers, 'install-transaction.py');builder = B.load(helpers, 'build-protected-core.py')
    contract = B.load(helpers, 'bootstrap-release.py');preflight = B.load(helpers, 'preflight-history-pilot.py')
    compose = B.load(helpers, 'compose-linux-host.py')

    def guards():
        B.need(time.monotonic() - started <= MAX_SECONDS, 'upgrade effect observation exceeded time budget')
        fence.lease(B, lock_fd);builder.worker_identity(B);builder.quiet_units(B);builder.idle(B)

    guards()
    state = journal.observe(B, helpers, manifest, manifest_pin, preservation, preservation_pin, lock_fd)
    B.need(state['head'] == code['revision'] and state['phase'] in ('revoked', 'retiring', 'retired'),
           'old run has not completed local revocation')
    old_raw = compose.retained_configuration(B, keep, preflight, contract, builder, preservation,
                                           preservation_pin, code['revision'], manifest_pin)
    config = json.loads(old_raw, object_pairs_hook=B.unique)
    saved = keep.verify(B, preservation, preservation_pin)
    saved_identity, saved_rows = journal.snapshot(B, keep, preservation / 'journal.sqlite3', live=False)
    same(B, saved_identity, saved['journal'], 'retained effect journal differs')
    original = keep.old_run(B, saved_rows)
    live_identity, live_rows = journal.snapshot(B, keep, PILOT / 'control/journal.sqlite3', live=True)
    B.need(digest(B.canonical(live_identity)) == state['journal_sha256'], 'journal advanced before effect observation')
    rows = [dict(zip(keep.TABLES['runs'], row)) for row in live_rows['runs'] if row[0] == keep.RUN]
    B.need(len(rows) == 1 and rows[0]['remote'] == 'revoked' and rows[0]['head'] == keep.HEAD,
           'upgrade effect run differs')
    run = rows[0];run = {**run, 'input': json.loads(run['input'], object_pairs_hook=B.unique)}
    payload = preflight.accepted_release(B, contract)
    client = preflight.client_class(payload)(config['app_id'], config['installation_id'], config['app_key'],
        config['repository'], permissions={'contents': 'read', 'checks': 'read'})
    try:
        transport_pin = transport(B, client, api_proxy)
        first = remote(B, client, config, run, contract, preflight);guards()
        retained = publication(B, keep, saved, preservation, config, original)
        same(B, remote(B, client, config, run, contract, preflight), first, 'remote state changed during effect observation')
        same(B, publication(B, keep, saved, preservation, config, original), retained,
             'retained publication changed during effect observation')
        same(B, journal.observe(B, helpers, manifest, manifest_pin, preservation, preservation_pin, lock_fd), state,
             'journal changed during effect observation')
        B.need(compose.retained_configuration(B, keep, preflight, contract, builder, preservation,
            preservation_pin, code['revision'], manifest_pin) == old_raw, 'retained effect configuration changed')
        same(B, keep.verify(B, preservation, preservation_pin), saved, 'effect preservation changed')
        B.authenticate(helpers, manifest, manifest_pin);guards()
    finally:
        client.token, client.expires = None, 0
    return {'schema': 'gatehouse-upgrade-effects/v1', 'state': 'observed', 'head': code['revision'],
        'helper_manifest_sha256': manifest_pin, 'preservation_sha256': preservation_pin,
        'api_transport_sha256': transport_pin,
        'old_run': keep.RUN, 'old_head': keep.HEAD, 'phase': state['phase'],
        'journal_sha256': state['journal_sha256'], 'remote': first, 'publication': retained,
        'remote_revocation_confirmed': True, 'retained_publication_verified': True,
        'installation_admitted': False, 'administrator_ready': False, 'production_gate_enabled': False,
        'scope': 'remote revocation/base and byte preservation only; preparation, storage, entry/drain and activation required'}


if __name__ == '__main__':
    raise SystemExit('upgrade effect observation is an internal library; no administrator entry is available')
