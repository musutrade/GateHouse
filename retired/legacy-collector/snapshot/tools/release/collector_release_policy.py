"""Independent tag eligibility; reuses Core CI authority without Core version equality."""
from __future__ import annotations

import argparse
import os
from pathlib import Path

import collector_assets as assets
import release_policy as core

ENVIRONMENT = 'rust-collector-release'
PERSONAL_REVIEWER = {'id': 23396976, 'login': 'higoalespn'}


def personal_environment_receipt():
    return {'name': ENVIRONMENT, 'required_reviewers': True,
            'prevent_self_review': False, 'can_admins_bypass': False,
            'approval_mode': 'single-maintainer', 'reviewer': dict(PERSONAL_REVIEWER)}


def protected_environment(value):
    rules = value.get('protection_rules', [])
    reviewers = [r for r in rules if r.get('type') == 'required_reviewers']
    assets.require(len(reviewers) == 1 and reviewers[0].get('reviewers')
                   and value.get('can_admins_bypass') is False,
                   'protected environment needs required reviewers and no admin bypass')
    rule = reviewers[0]
    if rule.get('prevent_self_review') is False:
        people = rule['reviewers']
        assets.require(value.get('name') == ENVIRONMENT and len(people) == 1
                       and people[0].get('type') == 'User'
                       and {key: people[0].get('reviewer', {}).get(key)
                            for key in PERSONAL_REVIEWER} == PERSONAL_REVIEWER,
                       'self-review requires the pinned single-maintainer environment')
        return personal_environment_receipt()
    assets.require(rule.get('prevent_self_review') is True, 'missing self-review policy')
    return {'name': ENVIRONMENT, 'required_reviewers': True,
            'prevent_self_review': True, 'can_admins_bypass': False}


def validate_receipt(receipt, tag, commit):
    assets.require(set(receipt) == {'schema', 'status', 'repository', 'tag', 'commit',
                                    'protected_main', 'ci', 'environment'}, 'invalid eligibility fields')
    assets.require(receipt['schema'] in ('rust-collector-eligibility/v1', 'rust-collector-eligibility/v2')
        and receipt['status'] == 'pass' and receipt['repository'] == assets.REPOSITORY
        and receipt['tag'] == tag and receipt['commit'] == commit
        and receipt['protected_main'] == 'refs/remotes/origin/main', 'invalid release eligibility')
    ci = receipt['ci']
    assets.require(ci['workflow'] == core.WORKFLOW_PATH
        and ci['aggregate_job'] == core.REQUIRED_AGGREGATE
        and type(ci['run_id']) is int and ci['run_id'] > 0
        and type(ci['aggregate_job_id']) is int and ci['aggregate_job_id'] > 0,
        'missing exact required CI receipt')
    expected = ({'name': ENVIRONMENT, 'required_reviewers': True,
                 'prevent_self_review': True, 'can_admins_bypass': False}
                if receipt['schema'] == 'rust-collector-eligibility/v1'
                else personal_environment_receipt())
    assets.require(receipt['environment'] == expected, 'unprotected release environment')


def verify(repo, manifest_path, tag, commit, client):
    manifest = assets.contract.load_manifest(manifest_path.read_bytes())
    assets.require(client.repository == assets.REPOSITORY, 'wrong release repository')
    assets.require(tag == 'rust-collector-v' + assets.version(manifest['collector']['version']),
                   'independent tag must exactly match collector manifest')
    assets.require('-rc.' in manifest['collector']['version'],
                   'stable promotion blocked pending separately accepted GH-215 integration')
    assets.require(commit == manifest['source_commit'], 'wrong manifest source commit')
    oid = core.verify_git_state(repo, tag, commit, 'refs/remotes/origin/main')
    ci = core.verify_ci_run(client, oid, 'main')
    environment = protected_environment(client.get_json(
        '/repos/' + assets.REPOSITORY + '/environments/' + ENVIRONMENT))
    schema = ('rust-collector-eligibility/v2' if environment.get('approval_mode') == 'single-maintainer'
              else 'rust-collector-eligibility/v1')
    receipt = {'schema': schema, 'status': 'pass',
               'repository': assets.REPOSITORY, 'tag': tag, 'commit': oid,
               'protected_main': 'refs/remotes/origin/main', 'ci': ci, 'environment': environment}
    validate_receipt(receipt, tag, oid)
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path.cwd())
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    client = core.GitHubClient('https://api.github.com', assets.REPOSITORY, os.environ.get('GH_TOKEN', ''))
    assets.write(args.output, verify(args.repo, args.manifest, args.tag, args.commit, client))


if __name__ == '__main__':
    main()
