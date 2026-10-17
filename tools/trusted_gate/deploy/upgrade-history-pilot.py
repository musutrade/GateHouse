#!/usr/bin/python3 -I
"""Fixed fa18 isolation-pilot upgrade; preserve evidence and never enable production."""
from __future__ import annotations

import argparse
import copy
import fcntl
import grp
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import pwd
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile

OLD_HEAD = 'fa18d094636c55e7f5b2937805f36a1bdf0f8e54'
OLD_SHA = '38b885ef92b8a661301b603bf6f1a7d697785ef37acae29052658600639b48d3'
OLD_INSTALLER_SHA = 'a8f7b95ceda1b5ba4e3f9f8faf9f662480cb525743eaecb4ea8e0522394e8486'
OLD_STAGE = Path('/root/gatehouse-issue98-pilot-fa18d09')
CONTROL = Path('/srv/gatehouse/pilot/control')
CONFIG = Path('/etc/gatehouse/host.json')
ALLOWED = {'trusted_gate/history.py', 'trusted_gate/isolation.py',
           'trusted_gate/source.py', 'bin/pilot-probe.py'}


def need(value, message):
    if not value:
        raise ValueError(message)


def owned(path, private=False):
    path = Path(path)
    for item in (path, *path.parents):
        info = item.lstat()
        need(info.st_uid == 0 and not stat.S_ISLNK(info.st_mode) and not info.st_mode & 0o022,
             'unprotected upgrade input: ' + str(item))
    if private:
        need(not path.stat().st_mode & 0o077, 'upgrade input is not private')
    return path


def bounded(path, limit=256 * 1024 * 1024, private=True):
    path = owned(path, private)
    need(path.is_file() and path.stat().st_size <= limit, 'upgrade input exceeds budget')
    return path.read_bytes()


def load(name, path, expected):
    data = bounded(path, 1024 * 1024)
    need(hashlib.sha256(data).hexdigest() == expected, 'installer differs from reviewed digest')
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def destination(member):
    if member.startswith('trusted_gate/') or member.startswith('bin/'):
        return Path('/opt/gatehouse') / member
    if member.startswith('policy/'):
        return Path('/etc/gatehouse') / member
    if member == 'srv-gatehouse-pilot.mount':
        return Path('/etc/systemd/system') / member
    if member == 'sudoers':
        return Path('/etc/sudoers.d/gatehouse-pilot')
    raise ValueError('member is not an installed file')


def plan(before, after, old_meta, new_meta):
    need(old_meta['head'] == OLD_HEAD and old_meta['coverage_acceptance'] ==
         new_meta['coverage_acceptance'] == 'transition', 'only the accepted fa18 transition pilot can be upgraded')
    need(new_meta.get('worker_git_history_probe') is True, 'candidate lacks the live worker-history probe')
    need(set(after) == set(before) | {'trusted_gate/history.py'}, 'unexpected upgrade inventory')
    changes = {name for name in after if before.get(name) != after[name]}
    need(changes - {'source.bundle', 'metadata.json'} <= ALLOWED, 'unreviewed installed-file change')
    return {str(destination(name)): {'before': before.get(name), 'after': after[name],
            'mode': 0o755 if name.startswith('bin/') or name == 'policy/gatehouse-step' else
                    0o440 if name == 'sudoers' else 0o644}
            for name in after if name not in ('source.bundle', 'metadata.json')}


def replace(path, data, mode):
    path = Path(path)
    owned(path.parent)
    if path.exists() or path.is_symlink():
        owned(path)
    fd, temporary = tempfile.mkstemp(prefix='.' + path.name + '-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            stream.flush()
            os.fchmod(stream.fileno(), mode)
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def write_json(path, value):
    replace(path, json.dumps(value, sort_keys=True, indent=2).encode() + b'\n', 0o600)


def expected_config(before, files, meta, digest):
    result = copy.deepcopy(before)
    result['environment'].update(source_revision=meta['head'], release_sha256=digest)
    result['steps']['pilot-boundary']['argv'] = ['/usr/bin/python3.14', '-I',
        '/opt/gatehouse/bin/pilot-probe.py', 'executor-history', meta['head'], meta['base_at_packaging']]
    result['steps']['pilot-boundary']['git_history'] = True
    for path, item in files.items():
        if path in result['protected_files'] or path == '/opt/gatehouse/trusted_gate/history.py':
            result['protected_files'][path] = hashlib.sha256(item['after']).hexdigest()
    return result


def publication_inventory(directory):
    owned(directory, True)
    result, total = {}, 0
    for root, directories, files in os.walk(directory):
        owned(root)
        for name in directories:
            owned(Path(root) / name)
        for name in files:
            path = Path(root) / name
            data = bounded(path, private=False)
            total += len(data)
            need(total <= 256 * 1024 * 1024 and len(result) < 20000, 'retained publication exceeds budget')
            result[str(path.relative_to(directory))] = hashlib.sha256(data).hexdigest()
    return result


def retained(journal, run, records, backup):
    count = journal.db.execute("SELECT count(*) FROM events WHERE run=? AND operation='retired'", (run['id'],)).fetchone()[0]
    need(run['remote'] == 'revoked' and count > 0, 'old source retirement is not proven')
    expected = json.loads(bounded(backup / 'publication-inventory.json', 4 * 1024 * 1024))
    need(publication_inventory(records / run['id'] / 'publication') == expected,
         'retained prior publication is incomplete or changed')
    for name, key in (('test_result.json', 'machine_sha256'), ('manifest.json', 'manifest_sha256'),
                      ('artifact-registry.json', 'registry_sha256')):
        data = bounded(records / run['id'] / 'publication' / name, 32 * 1024 * 1024, private=False)
        need(hashlib.sha256(data).hexdigest() == run['evidence'][key], 'retained prior evidence differs')


def upgrade(installer, payload, meta, digest, before, old_meta):
    files = plan(before, payload, old_meta, meta)
    owned(CONTROL, True)
    # The same lock fences all delegated operations throughout retirement/install.
    with (CONTROL / 'controller.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        active = installer.command(['/usr/bin/systemctl', 'list-units', '--all', '--plain',
            '--no-legend', '--no-pager', 'gatehouse-core-*', 'gatehouse-step-*'], capture=True)
        for line in active.stdout.decode().splitlines():
            fields = line.split()
            need(len(fields) >= 4 and fields[2] in ('inactive', 'failed'), 'a Core/worker unit is active or unknown')
            group = installer.command(['/usr/bin/systemctl', 'show', '--property=ControlGroup', '--value', fields[0]], capture=True).stdout.decode().strip()
            if group:
                need(group.startswith('/') and '..' not in Path(group).parts, 'unknown service cgroup path')
                events = Path('/sys/fs/cgroup') / group.lstrip('/') / 'cgroup.events'
                need(events.is_file() and 'populated 0' in events.read_text(), 'a service cgroup is not proven empty')
        users = [pwd.getpwnam('gatehouse-developer'), pwd.getpwnam('gatehouse-executor')]
        for user in users:
            groups = {grp.getgrgid(g).gr_name for g in os.getgrouplist(user.pw_name, user.pw_gid)}
            need(user.pw_uid > 0 and not groups & {'root', 'sudo', 'docker', 'lxd', 'adm', 'disk'},
                 'restricted identity gained privileged groups')
        need(users[0].pw_uid != users[1].pw_uid and users[1].pw_shell == '/usr/sbin/nologin', 'worker identity changed')
        processes = subprocess.run(['/usr/bin/pgrep', '-u', ','.join(str(u.pw_uid) for u in users)],
            env=installer.SAFE_ENV, capture_output=True, timeout=10, check=False)
        need(processes.returncode == 1, 'restricted identity processes must stop before upgrade')
        backup = CONTROL / ('history-upgrade-' + digest)
        intent = backup / 'intent.json'
        if backup.exists():
            record = json.loads(bounded(intent, 1024 * 1024))
            need(record['old_release'] == OLD_SHA and record['new_release'] == digest
                 and record['phase'] in ('prepared', 'retired', 'installed'), 'unknown upgrade state; retain it for reconciliation')
            old_config_raw = bounded(backup / 'host.json', 1024 * 1024)
            acceptance = json.loads(bounded(backup / 'acceptance.json', 1024 * 1024))
            old_marker = json.loads(bounded(backup / 'marker.json', 1024 * 1024))
        else:
            old_config_raw = bounded(CONFIG, 1024 * 1024)
            acceptance = json.loads(bounded(CONTROL / 'pilot-acceptance.json', 1024 * 1024))
            old_marker = json.loads(bounded(installer.MARKER, 1024 * 1024))
            record = {'old_release': OLD_SHA, 'new_release': digest, 'phase': 'prepared'}
        old_config = json.loads(old_config_raw)
        need(users[0].pw_uid == old_config['developer_uid'], 'accepted developer identity changed')
        need(old_config['coverage_acceptance'] == 'transition'
             and old_config['environment']['source_revision'] == OLD_HEAD
             and old_config['environment']['release_sha256'] == OLD_SHA
             and old_marker['head'] == OLD_HEAD and old_marker['release_sha256'] == OLD_SHA
             and acceptance['head'] == OLD_HEAD and acceptance['release_sha256'] == OLD_SHA
             and acceptance['production_gate_enabled'] is False
             and acceptance['authoritative_publish'] == 'blocked'
             and acceptance['coverage_acceptance'] == 'transition'
             and acceptance['negative_conclusion'] == 'failure', 'installed pilot identity differs')
        pins = {path: hashlib.sha256(item['before']).hexdigest() for path, item in files.items()
                if item['before'] is not None and path.startswith(('/opt/gatehouse/', '/etc/gatehouse/policy/'))}
        for path in ('/usr/bin/python3.14', '/usr/bin/git'):
            pins[path] = hashlib.sha256(bounded(path, private=False)).hexdigest()
        need(old_config['protected_files'] == pins, 'accepted installed input pins changed')
        for name, original in (('checks', '/home/gem/.secrets/my-disposable-bot.2026-09-08.private-key.pem'),
                               ('delivery', '/home/gem/.secrets/symphony-delivery.private-key.pem')):
            path = Path(original)
            info = path.lstat()
            need(stat.S_ISREG(info.st_mode) and info.st_uid == pwd.getpwnam('gem').pw_uid
                 and info.st_size < 32768, 'operator signing-key source changed')
            need(path.read_bytes() == bounded('/etc/gatehouse/' + name + '.private-key.pem', 32768),
                 'key rotation requires separate reconciliation; keys are never upgraded here')
        mounted = installer.command(['/usr/bin/findmnt', '-n', '-o', 'UUID', '--target', str(installer.VOLUME)], capture=True)
        need(mounted.stdout.decode().strip() == old_marker['volume_uuid'], 'pilot volume identity changed')
        for path, item in files.items():
            file = Path(path)
            if not file.exists() and not file.is_symlink() and item['before'] is None:
                continue
            actual = bounded(file, private=False)
            permitted = (item['before'],) if record['phase'] == 'prepared' else (item['before'], item['after'])
            need(actual in permitted, 'installed file differs from the reviewed upgrade: ' + path)
        # Verify unaffected toolchain pins before importing installed controller code.
        for path, expected in old_config['protected_files'].items():
            if path not in files:
                need(hashlib.sha256(bounded(path, private=False)).hexdigest() == expected, 'pinned toolchain changed')
        new_config = expected_config(old_config, files, meta, digest)
        need(bounded('/etc/gatehouse/pilot.sudoers', private=False) == before['sudoers'],
             'sudoers review input changed')
        current_config = json.loads(bounded(CONFIG, 1024 * 1024))
        need(current_config in (old_config, new_config), 'unknown live configuration; retain upgrade state')
        sys.path.insert(0, '/opt/gatehouse')
        from trusted_gate.journal import Journal
        from trusted_gate.execution import Executor
        from trusted_gate.github import GitHub
        from trusted_gate.control import Control
        from trusted_gate.source import retire
        journal = Journal(CONTROL / 'journal.sqlite3')
        try:
            run = journal.get(acceptance['run'])
            canonical_config = json.dumps(old_config, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
            need(hashlib.sha256(canonical_config).hexdigest() == run['input']['policy'],
                 'host policy changed since the accepted invocation')
            need(run['head'] == OLD_HEAD and run['pr'] is None and run['check_id'] is None
                 and run['remote'] in ('pending', 'revoking', 'revoked'), 'old pilot was attached/published or changed')
            executor = Executor(old_config, CONFIG)
            executor.confirm_stopped(run)
            github = GitHub(old_config['app_id'], old_config['installation_id'], old_config['app_key'], old_config['repository'])
            need(github.repo('GET', '')['id'] == old_config['repository_id'] and
                 github.repo('GET', '/git/ref/heads/' + old_config['base_branch'])['object']['sha'] == meta['base_at_packaging'],
                 'repository/base changed before upgrade; prepare a new reviewed candidate')
            outstanding = journal.outstanding()
            if record['phase'] != 'installed':
                need(all(item['id'] == run['id'] for item in outstanding), 'another run owns the publication slot')
            else:
                need(all(item['head'] == meta['head'] and item['local'] in ('pending', 'passed')
                         for item in outstanding), 'new invocation failed/running/unknown; explicit reconciliation required')
            if not backup.exists():
                executor.policy_identity()
                need(run['local'] == 'passed', 'accepted old pilot no longer has its recorded result')
                executor.verify_evidence(run, run['evidence'])
                installer.directory(backup, 0o700)
                installer.install(backup / 'host.json', old_config_raw, 0o600)
                installer.install(backup / 'acceptance.json', bounded(CONTROL / 'pilot-acceptance.json', 1024 * 1024), 0o600)
                installer.install(backup / 'marker.json', bounded(installer.MARKER, 1024 * 1024), 0o600)
                write_json(intent, record)
            if record['phase'] == 'prepared':
                need(executor.policy_identity() == run['input']['policy'], 'old installed policy changed during recovery')
                from trusted_gate.common import read_submission
                original_bundle = Path(old_config['inbox']) / 'candidate.bundle'
                need(read_submission(original_bundle, old_config['inbox'], old_config['developer_uid'],
                     old_config['max_bundle_bytes']) == before['source.bundle'], 'old inbox candidate changed')
                if Path(old_config['source']).exists():
                    # Retire's existing recovery path must never skip a partial
                    # prior copy. Seal and check the complete copy before removal.
                    executor.verify_evidence(run, run['evidence'])
                    source_publication = Path(run['evidence']['directory'])
                    target_publication = CONTROL / run['id'] / 'publication'
                    expected_publication = publication_inventory(source_publication)
                    if not target_publication.exists():
                        shutil.copytree(source_publication, target_publication, symlinks=False)
                    need(publication_inventory(target_publication) == expected_publication,
                         'partial prior publication copy; preserve source and reconcile')
                    write_json(backup / 'publication-inventory.json', expected_publication)
                    control = Control(journal, github, executor, old_config)
                    control.revoke(run['id'], 'administrator-reviewed worker-history pilot upgrade')
                    retire(old_config, executor, journal, run['id'])
                run = journal.get(run['id'])
                retained(journal, run, CONTROL, backup)
                record['phase'] = 'retired'
                write_json(intent, record)
            retained(journal, journal.get(run['id']), CONTROL, backup)
            if record['phase'] == 'retired':
                bundle = Path(old_config['inbox']) / 'candidate.bundle'
                if bundle.exists():
                    from trusted_gate.common import read_submission
                    captured = read_submission(bundle, old_config['inbox'], old_config['developer_uid'], old_config['max_bundle_bytes'])
                    if captured == before['source.bundle']:
                        saved = backup / 'candidate-before-upgrade.bundle'
                        need(not saved.exists(), 'old bundle backup already exists; reconcile')
                        os.replace(bundle, saved)
                        os.chown(saved, 0, 0)
                        os.chmod(saved, 0o600)
                    else:
                        need(captured == payload['source.bundle'], 'inbox candidate changed')
                current_marker = json.loads(bounded(installer.MARKER, 1024 * 1024))
                new_marker = {**old_marker, 'head': meta['head'], 'release_sha256': digest}
                need(current_marker in (old_marker, new_marker), 'another release owns the volume')
                write_json(installer.MARKER, new_marker)
                strict_install = installer.install
                def reviewed_install(path, data, mode=0o644):
                    path = Path(path)
                    if str(path) in files:
                        need(mode == files[str(path)]['mode'], 'unreviewed installed-file permissions')
                    if path == CONFIG:
                        need(mode == 0o600, 'host configuration must remain private')
                        need(json.loads(data) == new_config, 'upgrade would change unrelated host configuration')
                        need(json.loads(bounded(path, 1024 * 1024)) in (old_config, new_config), 'host configuration changed')
                        replace(path, data, mode)
                    elif str(path) in files and path.exists() and path.read_bytes() != data:
                        item = files[str(path)]
                        need(data == item['after'] and bounded(path, private=False) in (item['before'], item['after']), 'unreviewed file replacement')
                        replace(path, data, mode)
                    else:
                        strict_install(path, data, mode)
                installer.install = reviewed_install
                try:
                    config = installer.setup(payload, meta, digest)
                finally:
                    installer.install = strict_install
                record['phase'] = 'installed'
                write_json(intent, record)
            else:
                need(current_config == new_config, 'installed upgrade configuration differs')
                config = installer.setup(payload, meta, digest)
        finally:
            journal.close()
    # The public developer ingress takes the same lock itself. A failed or unknown
    # invocation remains blocked; the journal never implicitly reruns it.
    return installer.accept(config, meta, digest)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--installer-sha256', required=True)
    args = parser.parse_args(argv)
    need(os.geteuid() == 0, 'administrator authentication required; no alternate elevation is attempted')
    os.umask(0o077)
    installer = load('history_pilot_installer', Path(__file__).with_name('install-pilot.py'), args.installer_sha256)
    old_installer = load('accepted_fa18_installer', OLD_STAGE / 'install-pilot.py', OLD_INSTALLER_SHA)
    before, old_meta = old_installer.release(bounded(OLD_STAGE / 'release.tar'), OLD_SHA)
    payload, meta = installer.release(bounded(args.release), args.sha256)
    print(json.dumps(upgrade(installer, payload, meta, args.sha256, before, old_meta), sort_keys=True))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError, tarfile.TarError) as error:
        print(json.dumps({'status': 'blocked', 'reason': str(error), 'production_gate_enabled': False}), file=sys.stderr)
        raise SystemExit(1)
