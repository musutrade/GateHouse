"""Pure reviewed Linux transition contract, not a runnable host configuration."""
from __future__ import annotations

import hashlib
import json
import tomllib

FLOW = 'tools/trusted_gate/deploy/linux-transition.flow.toml'
INVENTORY = 'tools/trusted_gate/deploy/linux-transition.policy.json'
COMMANDS = ('nextest-version', 'nextest', 'findings-feedback', 'report-artifact', 'html-transport',
            'workflow-acceptance', 'schema-flow', 'schema-quality', 'schema-diff', 'onboarding')
COVERAGE = ['builtin.secret-scan', 'builtin.architecture-audit', 'pilot-boundary', 'pilot-host-contracts', 'linux-test']


def flow():
    steps = []
    for identity, component, label, timeout in (
            ('pilot-boundary', 'pilot', 'Live restricted worker boundary; transition only', 60),
            ('pilot-host-contracts', 'pilot', 'Host transport contracts; transition only', 120),
            ('linux-test', 'linux-test', 'Complete Linux Test workload; remaining project coverage pending', 1320)):
        steps.append({'id': identity, 'label': label, 'component': component, 'profiles': ['full', 'hook'],
                      'program': 'gatehouse-step', 'args': [identity], 'cwd': '{root}',
                      'log': identity + '.log', 'timeout_secs': timeout})
    return {'version': 2, 'project': {'name': 'gatehouse-linux-test-transition',
            'default_profile': 'full', 'hook_profile': 'hook'},
            'paths': {'reports': '.gatehouse-host/reports', 'audit_config': '.gatehouse-host/audit.toml',
                      'secrets_config': '.gatehouse-host/secrets.toml'},
            'execution': {'parallel': False},
            'policy': {'required_steps': ['pilot-boundary', 'pilot-host-contracts', 'linux-test']},
            'scope': {'unmatched': 'all', 'rules': [{'patterns': ['**'], 'components': ['pilot', 'linux-test']}]},
            'steps': steps}


def worker():
    captures = ['linux-test/result.json', 'linux-test/project-input-preparation.json',
                'linux-test/html-export/provenance.json', 'linux-test/html-export/test_result.html',
                'linux-test/project/target/quality/workflow-acceptance/receipt.json',
                'linux-test/project/target/quality/workflow-acceptance/summary.json',
                'linux-test/onboarding/report.json']
    captures += ['linux-test/' + name + '.' + stream for name in COMMANDS for stream in ('stdout', 'stderr')]
    captures += ['linux-test/onboarding/' + str(index).zfill(2) + '.' + stream + '.log'
                 for index in range(1, 30) for stream in ('stdout', 'stderr')]
    return {'argv': ['/usr/bin/python3.14', '-I', '/opt/gatehouse/linux-test/run-linux-test.py',
                    '--source', '/work/source', '--history', '/work/history',
                    '--cache', '/work/inputs/test-data-archives', '--output', '/work/output/linux-test',
                    '--onboarding-source', '/work/inputs/onboarding', '--head', '@gatehouse:head@',
                    '--base', '@gatehouse:base@', '--manifest-sha256', '@gatehouse:test-data-sha256@',
                    '--identity', '@gatehouse:receipt@'],
            'run_arguments': True, 'git_history': True, 'timeout_seconds': 1260,
            'capture_files': captures, 'env': {'CARGO_BUILD_JOBS': '4', 'NEXTEST_TEST_THREADS': '4'},
            'cache_inputs': {'dependencies': ['tools/harness-gate/Cargo.lock'],
                            'configuration': ['tools/harness-gate/Cargo.toml', '.cargo/config.toml',
                                              'tools/harness-gate/.cargo/config.toml', '.config/nextest.toml',
                                              '.github/workflows/ci.yml', 'test-data/manifest.json']}}


def inventory(flow_raw):
    return {'schema': 'gatehouse-linux-transition-policy/v1', 'coverage_acceptance': 'transition',
            'policy_scope': 'linux-test-only-not-full-project', 'flow_sha256': hashlib.sha256(flow_raw).hexdigest(),
            'profiles': ['full', 'hook'], 'coverage': COVERAGE,
            'builtin_gates': 'Core-default-secret-scan-and-architecture-audit',
            'worker_template': worker(),
            'required_readonly_mounts': {'runtime': '/opt/gatehouse/toolchain',
                'cargo_config': '/work/output/cargo-home/config.toml',
                'directory_sources': '/work/dependencies/sources',
                'test_data_archives': '/work/inputs/test-data-archives', 'onboarding': '/work/inputs/onboarding'},
            'host_configuration_required': ['toolchain_files', 'cache_identity', 'protected_files',
                'runtime-inventory-and-tool-lock', 'cargo-config-and-directory-inventory',
                'test-data-archives-and-onboarding-pins', 'verified-host-bindings', 'fixed-995-identity'],
            'candidate_resource_limits': {'timeout_seconds': 1800, 'executor_memory_bytes': 8 * 1024**3,
                'executor_file_bytes': 1024**3, 'storage_bytes': 16 * 1024**3},
            'pending_acceptance': ['live-Root-and-UID995-Linux-Test', 'other-Linux-quality-cells',
                'native-macOS-and-Windows', 'cold-warm-cache-and-resource-failure',
                'installation-startup-and-recovery', 'dedicated-App-rules-and-delivery'],
            'host_configuration_complete': False, 'captured_core_policy_checked': False,
            'authoritative_publish': 'blocked', 'administrator_ready': False, 'production_gate_enabled': False}


def validate(B, flow_raw, inventory_raw):
    # Compare canonical representations so True cannot stand in for integer 1.
    parsed = tomllib.loads(flow_raw.decode('utf-8'))
    B.need(B.canonical(parsed) == B.canonical(flow()), 'Linux transition flow differs from reviewed policy')
    parsed_inventory = json.loads(inventory_raw, object_pairs_hook=B.unique)
    B.need(inventory_raw == B.canonical(parsed_inventory) == B.canonical(inventory(flow_raw)),
           'Linux transition inventory differs or grants unaccepted authority')
    return {'coverage': list(COVERAGE), 'host_configuration_complete': False,
            'administrator_ready': False, 'production_gate_enabled': False}
