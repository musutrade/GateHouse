"""Trusted Core execution over a root-owned candidate and fixed, protected policy."""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import time
import tomllib
import urllib.parse

from .common import atomic_json, canonical, checked, digest, protected, read_json, read_regular, require, sha


HOST_DIRECTORY = '.gatehouse-host'


def clean_environment():
    # No Agent environment, Python paths, Git config, credentials or compiler wrappers.
    return {'PATH': '/usr/local/bin:/usr/bin:/bin', 'LANG': 'C.UTF-8',
            'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null',
            'GIT_TERMINAL_PROMPT': '0', 'GIT_LFS_SKIP_SMUDGE': '1'}


@checked
def confirm_units_stopped(*patterns):
    result = subprocess.run(['/usr/bin/systemctl', 'list-units', '--all', '--plain', '--no-legend',
        '--no-pager', *patterns], env=clean_environment(), capture_output=True, check=True, timeout=10)
    for line in result.stdout.decode().splitlines():
        fields = line.split()
        require(len(fields) >= 4 and fields[2] in ('inactive', 'failed'),
                'executor cgroup still active or unknown; retain slot and source')
        group = subprocess.run(['/usr/bin/systemctl', 'show', '--property=ControlGroup', '--value', fields[0]],
            env=clean_environment(), capture_output=True, check=True, timeout=10).stdout.decode().strip()
        if group:
            require(group.startswith('/') and '..' not in Path(group).parts, 'unsafe executor cgroup path')
            events = Path('/sys/fs/cgroup') / group.lstrip('/') / 'cgroup.events'
            require(events.is_file() and 'populated 0' in events.read_text().splitlines(),
                    'executor cgroup is not proven empty')


@checked
def git(root, *args):
    result = subprocess.run(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
        '-c', 'protocol.ext.allow=never', '-c', 'protocol.file.allow=never', *args],
        cwd=root, env=clean_environment(), capture_output=True, check=True, timeout=60)
    require(len(result.stdout) <= 16 * 1024 * 1024, 'Git response exceeds inventory budget')
    return result.stdout.decode('utf-8').strip()


class Executor:
    def __init__(self, config, configuration_path=None):
        self.config = config
        self.configuration_path = configuration_path
        self.configuration_digest = digest(read_regular(protected(configuration_path, private=True), 1024 * 1024)) if configuration_path else None
        self.source = Path(config['source'])
        protected(self.source.parent, directory=True)
        self.policy = protected(config['policy_directory'], directory=True)
        self.core = protected(config['core'])
        self.records = protected(config['records'], directory=True, private=True)

    @checked
    def policy_identity(self):
        if self.configuration_path:
            require(digest(read_regular(protected(self.configuration_path, private=True), 1024 * 1024)) == self.configuration_digest,
                    'host configuration changed during the operation')
        files = self.config['protected_files']
        require(isinstance(files, dict) and files and str(self.core) in files, 'Core and trusted inputs must be pinned')
        require(all(str(path) in files for path in Path(__file__).parent.glob('*.py')),
                'every installed controller module must be pinned')
        for name, expected in files.items():
            path = protected(name)
            require(digest(read_regular(path, 128 * 1024 * 1024)) == expected,
                    'installed trusted input differs from reviewed pin')
        return digest(canonical(self.config))

    @checked
    def validate_inputs(self, inputs):
        protected(self.source, directory=True)
        for name in ('head', 'tree', 'base'):
            sha(inputs[name])
        require(inputs['repository'] == self.config['repository'], 'wrong repository')
        require(git(self.source, 'rev-parse', 'HEAD') == inputs['head'], 'candidate HEAD changed')
        require(git(self.source, 'rev-parse', 'HEAD^{tree}') == inputs['tree'], 'candidate tree changed')
        git(self.source, 'merge-base', '--is-ancestor', inputs['base'], inputs['head'])
        require(not git(self.source, 'status', '--porcelain', '--untracked-files=no'), 'tracked candidate bytes changed')
        tracked = git(self.source, 'ls-tree', '-r', '--name-only', inputs['head']).splitlines()
        require(not any(name == HOST_DIRECTORY or name.startswith(HOST_DIRECTORY + '/') for name in tracked),
                'candidate occupies the reserved trusted policy directory')
        # Every source inode must remain outside the executor identity's write permissions.
        for root, directories, files in os.walk(self.source):
            if Path(root) == self.source:
                directories[:] = [d for d in directories if d not in ('.git', HOST_DIRECTORY)]
            for name in (*directories, *files):
                path = Path(root) / name
                require(not path.is_symlink(), 'source symlinks require a separately reviewed materializer')
                info = path.stat()
                require(info.st_uid == 0 and not info.st_mode & 0o022, 'candidate is writable outside the trusted host')
        require(inputs['policy'] == self.policy_identity(), 'policy changed')

    def execute(self, run):
        self.validate_inputs(run['input'])
        overlay = self.source / HOST_DIRECTORY
        require(not overlay.exists(), 'previous execution overlay must be retained/reconciled before a new run')
        shutil.copytree(self.policy, overlay, symlinks=False)
        flow_path = overlay / 'flow.toml'
        flow = tomllib.loads(read_regular(flow_path, 1024 * 1024).decode())
        require(not flow.get('execution', {}).get('parallel', False),
                'this host serializes protected steps; parallel execution needs independent cache leases')
        require(flow['project']['default_profile'] == self.config['profile'] == 'full', 'only full verification is accepted')
        require(flow['paths']['reports'] == HOST_DIRECTORY + '/reports', 'reports must stay in the protected overlay')
        expected = self.config['coverage']
        require(expected and len(expected) == len(set(expected)), 'coverage inventory is empty or ambiguous')
        # All commands run through one pinned sandbox launcher. Built-in secret/audit gates stay in Core.
        launcher = str(self.policy / 'gatehouse-step')
        require(launcher in self.config['protected_files'], 'sandbox launcher is not pinned')
        for step in flow['steps']:
            if step.get('kind') != 'builtin-gate':
                require(step.get('program') == 'gatehouse-step' and step.get('args') == [step['id']],
                        'project commands must use the protected sandbox launcher and fixed step ID')
                require(not step.get('runner') and not step.get('services'),
                        'runner/services require a separately reviewed protected execution integration')
            else:
                require(step.get('gate_type') in ('secret-scan', 'architecture-audit'), 'unknown trusted builtin')
        output = self.records / run['id']
        output.mkdir(mode=0o700)
        atomic_json(output / 'configuration.json', self.config)
        started = time.time()
        # systemd owns the entire trusted Core cgroup. An outer process crash does not authorize
        # another run; the journal stays running until the operator proves the cgroup is stopped.
        argv = ['/usr/bin/systemd-run', '--quiet', '--wait', '--pipe', '--collect',
                '--unit=gatehouse-core-' + run['id'], '--property=RuntimeMaxSec=' + str(self.config['timeout_seconds']),
                '--property=KillMode=control-group', '--property=UMask=0077',
                '--setenv=PATH=' + str(self.policy) + ':/usr/local/bin:/usr/bin:/bin',
                '--setenv=GATEHOUSE_RUN_ID=' + run['id'],
                str(self.core), '--project-root', str(self.source), '--config', str(flow_path),
                'verify', '--profile', 'full', '--all']
        env = clean_environment()
        env['GATEHOUSE_RUN_ID'] = run['id']
        with (output / 'core.log').open('xb') as log:
            result = subprocess.run(argv, env=env, stdout=log, stderr=subprocess.STDOUT,
                                    timeout=self.config['timeout_seconds'] + 60)
        require(result.returncode == 0, 'Core full verification did not complete successfully')
        self.validate_inputs(run['input'])
        reports = overlay / 'reports'
        machine = read_json(reports / 'test_result.json', 32 * 1024 * 1024)
        invocation = reports / 'invocations' / machine['invocation_id']
        require(invocation.parent == reports / 'invocations', 'unsafe invocation identity')
        evidence = {'directory': str(invocation), 'machine_sha256': digest(read_regular(invocation / 'test_result.json', 32 * 1024 * 1024)),
                    'manifest_sha256': digest(read_regular(invocation / 'manifest.json', 1024 * 1024)),
                    'registry_sha256': digest(read_regular(invocation / 'artifact-registry.json', 1024 * 1024)),
                    'configuration_sha256': digest(read_regular(output / 'configuration.json', 1024 * 1024)),
                    'started': started, 'finished': time.time(),
                    'valid_until': time.time() + self.config['freshness_seconds'], 'run': run['id']}
        prefix = self.config.get('evidence_url_prefix')
        if prefix:
            url = urllib.parse.urlsplit(prefix)
            require(url.scheme == 'https' and url.hostname and not url.username and not url.password
                    and not url.query and not url.fragment and prefix.endswith('/'), 'unsafe evidence URL prefix')
            evidence['url'] = prefix + run['id']
        self.verify_evidence(run, evidence)
        atomic_json(output / 'local-evidence.json', evidence)
        return evidence

    @checked
    def verify_evidence(self, run, evidence):
        require(evidence['run'] == run['id'], 'evidence belongs to another gate run')
        if evidence.get('configuration_sha256'):
            configuration = self.records / run['id'] / 'configuration.json'
            raw_configuration = read_regular(protected(configuration, private=True), 1024 * 1024)
            require(digest(raw_configuration) == evidence['configuration_sha256'] and
                    digest(canonical(read_json(configuration))) == run['input']['policy'],
                    'retained host configuration does not match this gate run')
        directory = protected(evidence['directory'], directory=True, private=True)
        require(directory.parent == self.source / HOST_DIRECTORY / 'reports/invocations', 'evidence is outside this candidate')
        raw = read_regular(directory / 'test_result.json', 32 * 1024 * 1024)
        manifest_raw = read_regular(directory / 'manifest.json', 1024 * 1024)
        require(digest(raw) == evidence['machine_sha256'] and digest(manifest_raw) == evidence['manifest_sha256'],
                'Core evidence changed')
        registry_raw = read_regular(directory / 'artifact-registry.json', 1024 * 1024)
        require(digest(registry_raw) == evidence['registry_sha256'], 'Core artifact registry changed')
        machine, manifest = read_json(directory / 'test_result.json', 32 * 1024 * 1024), read_json(directory / 'manifest.json')
        registry = read_json(directory / 'artifact-registry.json')
        metadata = read_json(directory / 'invocation.json')
        require(machine['schema_version'] == '1' and manifest['schema_version'] == '1'
                and registry['schema_version'] == '1'
                and machine['invocation_id'] == metadata['invocation_id'] == manifest['invocation_id'] == registry['invocation_id'] == directory.name,
                'Core publication identity mismatch')
        require(metadata['commit'] == run['head'] and machine['profile'] == 'full'
                and machine['evidence_complete'] is True and machine['status'] == 'PASS'
                and machine['passed'] is True and not machine['failures'], 'Core has not authorized full delivery')
        require(metadata['profile'] == 'full' and metadata['input_mode'] == machine['input_mode'] == 'all'
                and metadata['source_identity'] == machine['source_identity'] == 'working-tree:' + run['head']
                and metadata['execution_root'] == machine['execution_root'] == str(self.source)
                and metadata['configuration_digest'] == machine['configuration_digest'],
                'Core invocation does not bind this complete candidate/configuration')
        observed = [step['step_id'] for step in machine['steps']]
        require(len(observed) == len(set(observed)) and set(observed) == set(self.config['coverage']),
                'required verification coverage is missing, duplicate or unexpected')
        require(not machine['skipped_steps'] and all(step['status'] == 'PASS' and step['passed'] is True
                and not step.get('waived') and not step.get('cancelled') and not step.get('timed_out')
                for step in machine['steps']), 'partial, skipped, waived or failed execution cannot authorize delivery')
        seen = set()
        total = 0
        for item in manifest['artifacts']:
            path = Path(item['path'])
            require(not path.is_absolute() and '..' not in path.parts and path.parts
                    and item['path'] not in seen, 'unsafe/duplicate manifest artifact')
            seen.add(item['path'])
            value = read_regular(directory / path, 256 * 1024 * 1024)
            total += len(value)
            require(total <= 256 * 1024 * 1024, 'Core publication exceeds evidence budget')
            require(len(value) == item['size_bytes'] and digest(value) == item['sha256'], 'artifact digest mismatch')
        require('invocation.json' in seen, 'manifest misses Core invocation metadata')
        for bindings in (machine['artifacts'], registry['artifacts']):
            require(all(item['invocation_id'] == directory.name for item in bindings), 'artifact invocation mismatch')
            projection = [{key: item[key] for key in ('path', 'kind', 'size_bytes', 'sha256')} for item in bindings]
            require(projection == manifest['artifacts'], 'Core machine/registry/manifest inventories differ')
        actual = set()
        for root, directories, files in os.walk(directory):
            if Path(root) == directory:
                directories[:] = [name for name in directories if name != 'isolation']
            for name in (*directories, *files):
                require(not (Path(root) / name).is_symlink(), 'publication contains a symbolic link')
            actual.update(str((Path(root) / name).relative_to(directory)) for name in files)
        require(actual == seen | {'test_result.json', 'manifest.json', 'artifact-registry.json'},
                'undeclared/missing publication files')

    @checked
    def confirm_stopped(self, run):
        confirm_units_stopped('gatehouse-core-' + run['id'] + '.service',
                              'gatehouse-step-' + run['id'] + '-*.service')
