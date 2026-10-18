"""Administrative service entry point; no agent-supplied command, policy or verdict."""
from __future__ import annotations

import argparse
from contextlib import ExitStack
import fcntl
import json
import os
from pathlib import Path
import re
import sys
import urllib.parse

from .common import Blocked, controller_configuration, protected, read_submission, require, submission_inbox
from .control import Control
from .execution import Executor
from .github import APIError, GitHub
from .isolation import launch, unprivileged_identity
from .journal import Journal
from .publication import deliver
from .source import prepare, retire
from .runtime import settings as runtime_settings


CONFIG = Path('/etc/gatehouse/host.json')


def configuration():
    require(os.geteuid() == 0, 'use the protected host service; a developer process cannot publish authoritative results')
    protected(Path(__file__))
    config = controller_configuration(protected(CONFIG, private=True))
    require(config.get('schema') == 'gatehouse-trusted-host/v1', 'unsupported host configuration')
    require(re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', config['repository']), 'invalid repository')
    require(re.fullmatch(r'[A-Za-z0-9_.-]+', config['base_branch']), 'base branch must be an explicit simple ref')
    require(config['profile'] == 'full', 'host accepts only the full profile')
    if config.get('coverage_acceptance') == 'complete':
        url = urllib.parse.urlsplit(config.get('evidence_url_prefix', ''))
        require(url.scheme == 'https' and url.hostname and not url.hostname.endswith('.invalid'),
                'complete mode requires an existing protected evidence endpoint')
    require(config['app_id'] != 15368, 'GitHub Actions App cannot be the dedicated host publisher')
    for field in ('records', 'mirror', 'scratch', 'policy_directory'):
        protected(config[field], directory=True)
    paths = [Path(config[key]) for key in ('records', 'mirror', 'scratch', 'policy_directory', 'source')]
    paths.append(Path(config['cache']['root']))
    paths.append(submission_inbox(config['inbox']))
    require(all(path.is_absolute() and '..' not in path.parts and not any(c.isspace() for c in str(path)) and ':' not in str(path)
                for path in paths), 'host paths must be absolute and unambiguous systemd mount operands')
    require(all(a != b and a not in b.parents and b not in a.parents
                for index, a in enumerate(paths) for b in paths[index + 1:]),
            'source, policy, control state, mirror, scratch, cache and inbox must be disjoint')
    require(config['freshness_seconds'] > 0 and config['timeout_seconds'] > 0, 'invalid time budget')
    runtime_settings(config)
    unprivileged_identity(config['developer_uid'])
    return config


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    create = sub.add_parser('validate')
    create.add_argument('--bundle', type=Path, required=True)
    create.add_argument('--head', required=True)
    attach = sub.add_parser('attach')
    attach.add_argument('run')
    attach.add_argument('--pr', type=int, required=True)
    delivery = sub.add_parser('deliver')
    delivery.add_argument('run')
    delivery.add_argument('--title', required=True)
    delivery.add_argument('--body-file', type=Path, required=True)
    for name in ('publish', 'revoke', 'retire'):
        command = sub.add_parser(name)
        command.add_argument('run')
        if name == 'revoke':
            command.add_argument('--reason', required=True)
    sub.add_parser('reconcile')
    sub.add_parser('status')
    sub.add_parser('maintain-cache')
    step = sub.add_parser('step')
    step.add_argument('step_id')
    args = parser.parse_args(argv)
    os.umask(0o077)
    try:
        config = configuration()
        with ExitStack() as resources:
            if args.action != 'step':
                lock = resources.enter_context((Path(config['records']) / 'controller.lock').open('a'))
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            # A process can load the old generation before obtaining the installation
            # lease. Recheck before any SQLite mutation, token or network activity.
            controller_configuration(CONFIG)
            journal = Journal(Path(config['records']) / 'journal.sqlite3')
            resources.callback(journal.close)
            if args.action == 'step':
                # Core is already under the controller's exclusive lock. The launcher may only
                # execute a fixed step belonging to that running invocation, never acquire a new slot.
                return launch(config, journal, os.environ.get('GATEHOUSE_RUN_ID', ''), args.step_id)
            executor = Executor(config, configuration_path=CONFIG)
            github = GitHub(config['app_id'], config['installation_id'], config['app_key'], config['repository'],
                            https_proxy=config.get('https_proxy'))
            control = Control(journal, github, executor, config)
            if args.action == 'validate':
                # Repeated exact requests recover the recorded run without executing again.
                old = journal.latest(config['repository'], args.head)
                if old and old['remote'] != 'revoked':
                    result = control.validate(old['input'])
                else:
                    payload = read_submission(args.bundle, config['inbox'], config['developer_uid'], config['max_bundle_bytes'])
                    inputs = prepare(config, github, journal, payload, args.head)
                    inputs['policy'] = executor.policy_identity()
                    result = control.validate(inputs)
            elif args.action == 'attach':
                result = control.attach(args.run, args.pr)
            elif args.action == 'deliver':
                settings = config['delivery']
                publisher = GitHub(settings['app_id'], settings['installation_id'], settings['app_key'],
                    config['repository'], permissions={'contents': 'write', 'pull_requests': 'write'},
                    https_proxy=config.get('https_proxy'))
                result = deliver(control, publisher, args.run, args.title,
                    read_submission(args.body_file, config['inbox'], config['developer_uid'], 16384).decode('utf-8'))
            elif args.action == 'publish':
                require(config.get('coverage_acceptance') == 'complete',
                        'coverage migration is not accepted; cannot publish an authoritative success')
                result = control.publish(args.run)
            elif args.action == 'revoke':
                result = control.revoke(args.run, args.reason)
            elif args.action == 'retire':
                result = retire(config, executor, journal, args.run)
            elif args.action == 'reconcile':
                result = control.reconcile()
            elif args.action == 'maintain-cache':
                require(not journal.outstanding(), 'cache maintenance requires an empty publication slot')
                from .cache import Cache
                settings = config['cache']
                protected(settings['root'], directory=True)
                result = Cache(settings['root'], settings['max_bytes'], settings['max_entries'],
                               settings['ttl_seconds']).maintain()
            else:
                result = journal.outstanding()
            print(json.dumps(result, sort_keys=True))
            return 0
    except (Blocked, APIError, OSError, KeyError, ValueError) as error:
        # API errors intentionally omit remote bodies and headers. No secret/config dump.
        print(json.dumps({'status': 'blocked', 'reason': str(error)}), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
