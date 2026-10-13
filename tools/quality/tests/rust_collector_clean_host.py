"""Phased GH-230 acceptance driver for the pinned operator container.

The harness and disposable signer are test inputs, separate from /runtime. Run
seed, prepare, collect and negatives in fresh container processes under strace.
Host orchestration signs requests between prepare and collect; no private signer
or source checkout is required by the delivered collector.
"""
import copy
import json
import os
from pathlib import Path
import shutil
import sys
import time

sys.path[:0] = ['/runtime/app', str(Path(__file__).parent), str(Path(__file__).parents[1])]
import rust_collector_delivery as delivery
import rust_collector_contract as contract
import rust_collector_project as project
import rust_native_driver as native
import rust_native_policy as policy
from native_policy_fixture import CRAP_RULE
import rust_collector_config_fixture as config
from test_rust_collector_project import binding_for, request_for
from test_rust_collector_runtime import StandaloneNativeTests

ROOT = Path('/work')
RUNTIME = Path('/runtime')
CORE = '/released-core/harness-gate-linux-amd64'
CORE_ID = {'version': '0.4.0', 'commit': 'ee544690662645806cda7dd4cd2c9192566f7929',
           'sha256': '8e3df8303ca8f650d4ef768b29cfefb60ca115122a47b116245cc19e4649bbdd'}


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def pinned(path):
    return {'path': str(path), 'sha256': native.file_hash(path)}


def setup():
    cls = StandaloneNativeTests
    cls.runtime, cls.work = RUNTIME, ROOT / 'captures'
    cls.command_index = len(list((cls.work / 'commands').glob('*.command.json')))
    cls.environment = {'PATH': '/runtime/bin', 'LD_LIBRARY_PATH': '/runtime/lib:/runtime/rust/lib',
        'TMPDIR': '/work/tmp', 'CARGO_HOME': '/work/cargo-home', 'CARGO_NET_OFFLINE': 'true'}
    cls.anchors = json.loads((ROOT / 'anchors.json').read_text()) if (ROOT / 'anchors.json').exists() else {}
    os.environ['HARNESS_GATE_NATIVE_POLICY_BINARY'] = CORE
    return cls()


def seed(test):
    assert not Path('/usr/bin/python3').exists()
    assert not Path('/mnt/dev-ssd/workspaces/symphony/GH-230/tools').exists()
    assert native.file_hash(Path(CORE)) == CORE_ID['sha256']
    for name in ('base', 'head'):
        anchor = native.collect_fixture(ROOT / 'driver/tools/quality/fixtures/rust-native/driver_complete.rs',
            test.work / name, RUNTIME / 'bin/harness-gate-rust-native-driver', RUNTIME / 'rust')
        test.anchors[name] = anchor
    write(ROOT / 'anchors.json', test.anchors)
    # Two full independent native re-exports, original bytes retained.
    first, second = test.measurement('head'), test.measurement('head')
    assert first == second
    write(ROOT / 'first-reexport.json', first)
    write(ROOT / 'second-reexport.json', second)
    # Actual published Core decides low coverage, debt and capability outcomes.
    test.test_private_generic_projection_is_decided_by_actual_core()
    write(ROOT / 'seed-receipt.json', {'scope': 'fixture-only; candidate for controller review',
        'core': CORE_ID, 'anchors': test.anchors,
        'reexports': [pinned(ROOT / name) for name in ('first-reexport.json', 'second-reexport.json')],
        'core_decision': pinned(test.work / 'generic-core/report.json'),
        'source_checkout_visible': False, 'ambient_python_present': False})


def prepare(test):
    test.collector_version = '0.0.0-rc.230'
    output = ROOT / 'configured'
    output.mkdir()
    root, state, state_path, request_path, request = config.prepare(test, CORE, output)
    binding_path = output / 'binding.json'
    binding = json.loads(binding_path.read_text())
    observed, paths = delivery.observe(RUNTIME, Path(CORE), CORE_ID)
    inventory = json.loads((RUNTIME / 'runtime.json').read_text())
    manifest = {'schema': 'rust-collector-delivery/v1', 'collector': binding['input']['collector'],
        'source_commit': CORE_ID['commit'], 'protocol': delivery.PROTOCOL,
        'host_abi': observed['host_abi'], 'core_compatibility': [CORE_ID],
        'tools': [dict(t, path=str(paths[t['name']].relative_to(RUNTIME))) for t in observed['tools']],
        'payloads': [{'path': str(p.relative_to(RUNTIME)), 'sha256': native.file_hash(p), 'role': 'runtime'}
                     for p in sorted(RUNTIME.rglob('*')) if p.is_file()],
        'capabilities': [dict(c, scope='single-file-fixture') for c in binding['capabilities']],
        'measurement': {'native_series': native.SERIES, 'normalized_series': [binding['series']['id']],
            'compiler_commit': native.RUSTC_COMMIT, 'llvm_version': '22.1.6',
            'compiler_inventory_schema': native.SCHEMA,
            'adapter_sha256': native.file_hash(RUNTIME / 'app/rust_native_driver.py'),
            'projection_sha256': native.file_hash(RUNTIME / 'app/rust_collector_project.py'),
            'classifier_sha256': native.file_hash(RUNTIME / 'app/rust_native_classify.py'),
            'normalization': 'exact owner counts and rational CRAP; rust_collector_project',
            'source_boundary': 'single-file-fixture production owners',
            'configuration_sha256': binding['config_digest']},
        'dependency_inventory_sha256': native.file_hash(RUNTIME / 'runtime.json'),
        'license_inventory_sha256': contract.fingerprint({k:v for k,v in inventory['payload'].items()
                                                        if k.startswith('licenses/')})}
    contract.validate_manifest(manifest)
    write(output / 'manifest.json', manifest)
    matrix = {'schema': 'rust-collector-compatibility/v1', 'tested': [{
        'manifest_sha256': contract.fingerprint(manifest), 'environment': observed,
        'receipt': dict(pinned(ROOT / 'seed-receipt.json'), kind='reviewed-native-capture-and-core-evaluation')}]}
    write(output / 'matrix.json', matrix)
    binding['delivery'] = {'manifest': pinned(output / 'manifest.json'),
        'matrix': pinned(output / 'matrix.json'), 'core_path': CORE, 'core': CORE_ID}
    request, _ = request_for(binding, binding_path)
    request['adapter'].update(executable='/runtime/bin/harness-gate-rust-collector',
        source_digest=native.file_hash(RUNTIME / 'bin/harness-gate-rust-collector'),
        signature={'algorithm': 'ed25519', 'key_id': 'configured-test', 'value': ''})
    request.update(nonce='clean-installed-positive', timeout_ms=120000, issued_at_ms=int(time.time()*1000))
    request['expires_at_ms'] = request['issued_at_ms'] + 3600000
    write(output / 'unsigned-request.json', request)


def collect(test):
    output = ROOT / 'configured'
    root = output / 'project'
    state_path = output / 'state.json'
    state = json.loads(state_path.read_text())
    request_path = root / '.harness-gate/native-request.json'
    request = json.loads((output / 'signed-request.json').read_text())
    write(request_path, request)
    config.pin(root, state, state_path)
    command = [CORE, 'quality', 'collect', '--repository-root', str(root), '--state', str(state_path),
        '--trusted-keys', str(output / 'trusted-keys.json'), '--output', str(output / 'collection.json')]
    stale = copy.deepcopy(request)
    stale['input']['context']['commit'] = 'f'*40
    write(request_path, stale)
    config.pin(root, state, state_path)
    result = test.command('clean-stale-context', command)
    test.assertNotEqual(result.returncode, 0)
    test.assertIn('collector roots/selection/capability request mismatch', result.stderr)
    write(request_path, request)
    config.pin(root, state, state_path)
    result = test.command('clean-installed-collect', command)
    test.assertEqual(result.returncode, 0, result.stderr)
    response = json.loads((output / 'collection.json').read_text())
    write(output / 'positive-collection.json', response)
    write(output / 'collection-shape.json', list(response))
    command[-1] = str(output / 'replay-collection.json')
    result = test.command('clean-installed-replay', command)
    test.assertNotEqual(result.returncode, 0)
    test.assertIn('collection requires a fresh artifact root or pinned retained artifacts', result.stderr)


def evaluate(test):
    """Evaluate the actual installed collection with a fresh certified baseline."""
    output = ROOT / 'configured'
    collected = json.loads((output / 'positive-collection.json').read_text())
    base = test.measurement('base')
    head = json.loads((ROOT / 'first-reexport.json').read_text())
    artifacts = output / 'baseline-artifacts'
    artifacts.mkdir()
    binding = binding_for(base, test.work / 'base', artifacts, '0.0.0-rc.230')
    binding['input']['context'].update(commit=collected['inputs']['expected']['base_commit'], base_commit='d' * 40)
    request, digest = request_for(binding, output / 'baseline-binding.json')
    binding = project.load_binding(request, output / 'baseline-binding.json', digest)
    projected = project.project_report(base, binding)
    projections = [dict(project=binding['project'], evidence=projected['collection']['evidence'],
                        expected=binding['input']['context']),
                   dict(project=collected['inputs']['project'], evidence=collected['evidence'],
                        expected=collected['inputs']['expected'])]
    declared, mappings, _ = policy.policy_and_lineage(base, head, *projections, ['legacy_debt'], CRAP_RULE)
    command = [CORE, 'quality', 'evaluate', '--output', str(output / 'installed-core-report.json')]
    for name, value in [('policy', declared), ('mappings', mappings)]:
        path = output / ('installed-' + name + '.json')
        write(path, value)
        command.extend(['--' + name, str(path)])
    for prefix, projection, source, artifact in zip(('base-', ''), projections,
            (test.work / 'base', output / 'project'), (artifacts, output / 'project/target/evidence')):
        for name, value in projection.items():
            path = output / ('installed-' + prefix + name + '.json')
            write(path, value)
            command.extend(['--' + prefix + name, str(path)])
        command.extend(['--' + prefix + 'source-root', str(source),
                        '--' + prefix + 'artifact-root', str(artifact)])
    result = test.command('clean-installed-core-evaluate', command)
    test.assertEqual(result.returncode, 1, result.stderr)
    report = json.loads((output / 'installed-core-report.json').read_text())
    test.assertEqual(report['aggregate']['state'], 'fail')
    test.assertNotIn('measurement_error', {g['state'] for g in report['gates'].values()})
    test.assertTrue(any(g['record']['head'].get('numerator') == 56 and g['state'] == 'fail'
                        for g in report['gates'].values()))


def malformed(test):
    output = ROOT / 'configured'
    artifacts = output / 'project/target/evidence'
    retained = output / 'malformed-retained-positive'
    shutil.move(artifacts, retained)
    artifacts.mkdir()
    try:
        malformed_requests(test)
    finally:
        # Keep unexpected writes for investigation rather than deleting them.
        if not any(artifacts.iterdir()):
            artifacts.rmdir()
            shutil.move(retained, artifacts)


def malformed_requests(test):
    output = ROOT / 'configured'
    root, state_path = output / 'project', output / 'state.json'
    state = json.loads(state_path.read_text())
    request_path = root / '.harness-gate/native-request.json'
    before = {str(p): native.file_hash(p) for p in (root / 'target/evidence').iterdir()}
    write(request_path, json.loads((output / 'malformed-sign/signed-request.json').read_text()))
    config.pin(root, state, state_path)
    command = [CORE, 'quality', 'collect', '--repository-root', str(root), '--state', str(state_path),
        '--trusted-keys', str(output / 'malformed-sign/trusted-keys.json'),
        '--output', str(output / 'malformed-collection.json')]
    for name, error in [('malformed', 'malformed adapter response:'),
                         ('malformed-replay', 'nonce has already been used')]:
        result = test.command('clean-' + name, command)
        test.assertNotEqual(result.returncode, 0)
        test.assertIn(error, result.stderr)
        test.assertFalse((output / 'malformed-collection.json').exists())
        test.assertEqual(before, {str(p): native.file_hash(p) for p in (root / 'target/evidence').iterdir()})


def negatives(test):
    output = ROOT / 'negatives'
    output.mkdir()
    original = json.loads((ROOT / 'configured/binding.json').read_text())
    unsigned = json.loads((ROOT / 'configured/unsigned-request.json').read_text())
    for name, message in [('unknown-combination', 'unknown/ambiguous tested'),
                          ('wrong-core', 'wrong released Core bytes'),
                          ('wrong-tool', 'wrong delivered tool path'),
                          ('relocated-capture', 'outside this private runtime'),
                          ('missing-owner', 'missing')]:
        binding = copy.deepcopy(original)
        artifacts = output / (name + '-artifacts')
        artifacts.mkdir()
        binding['input']['output_root'] = str(artifacts)
        if name in ('unknown-combination', 'wrong-tool'):
            matrix = json.loads((ROOT / 'configured/matrix.json').read_text())
            if name == 'unknown-combination':
                matrix['tested'] = []
            else:
                manifest = json.loads((ROOT / 'configured/manifest.json').read_text())
                # Point a tool at another inventoried real payload and bind its
                # hash: shape-valid, but not the actual executed tool path.
                manifest['tools'][0].update(path='bin/harness-gate-rust-collector',
                    sha256=native.file_hash(RUNTIME / 'bin/harness-gate-rust-collector'))
                write(output / 'wrong-tool-manifest.json', manifest)
                binding['delivery']['manifest'] = pinned(output / 'wrong-tool-manifest.json')
            write(output / (name + '-matrix.json'), matrix)
            binding['delivery']['matrix'] = pinned(output / (name + '-matrix.json'))
        elif name == 'wrong-core':
            binding['delivery']['core']['sha256'] = '0'*64
        elif name == 'relocated-capture':
            directory = output / name
            shutil.copytree(test.work / 'head', directory)
            capture = json.loads((directory / 'capture.json').read_text())
            capture['tools']['rustc']['path'] = '/relocated/rust/bin/rustc'
            write(directory / 'capture.json', capture)
            binding['capture']['path'] = str(directory)
        else:
            binding['project']['subjects'] = []
        path = output / (name + '-binding.json')
        request, _ = request_for(binding, path)
        request['adapter'] = unsigned['adapter']
        request['timeout_ms'] = 120000
        result = test.entry('clean-' + name, *request['args'], data=json.dumps(request))
        test.assertEqual(result.returncode, 1, result.stderr)
        response = json.loads(result.stdout)
        test.assertIn(message, response['collection']['error'])
        test.assertEqual(response['collection']['evidence'], [])
        test.assertEqual(list(artifacts.iterdir()), [])
    # A structurally valid selected owner that the real native export does not
    # contain must also fail after complete re-export, before any projection.
    import project_model as model
    binding = copy.deepcopy(original)
    artifacts = output / 'absent-native-owner-artifacts'
    artifacts.mkdir()
    binding['input']['output_root'] = str(artifacts)
    subject = binding['project']['subjects'][0]
    old = subject['id']
    subject['discriminator'] = 'f' * 64
    subject['id'] = model.subject_id(binding['project']['id'], subject)
    for claim in binding['input']['bindings']:
        if claim['subject'] == old:
            claim['subject'] = subject['id']
    request, _ = request_for(binding, output / 'absent-native-owner-binding.json')
    request['adapter'] = unsigned['adapter']
    result = test.entry('clean-absent-native-owner', *request['args'], data=json.dumps(request))
    test.assertEqual(result.returncode, 1, result.stderr)
    test.assertIn('missing selected native owner', json.loads(result.stdout)['collection']['error'])
    test.assertEqual(list(artifacts.iterdir()), [])


def cost(test):
    """Sample allocated/logical workspace disk every 10ms during real work."""
    import threading
    directory = ROOT / 'cost'
    directory.mkdir()
    records = []

    def measured(label, operation):
        stop = threading.Event()
        peak = {'logical_bytes': 0, 'allocated_bytes': 0, 'samples': 0}
        def sample():
            logical = allocated = 0
            seen = set()
            for path in directory.rglob('*'):
                try:
                    st = path.stat()
                except FileNotFoundError:
                    continue
                if not path.is_file() or (st.st_dev, st.st_ino) in seen:
                    continue
                seen.add((st.st_dev, st.st_ino))
                logical += st.st_size
                allocated += st.st_blocks * 512
            peak['logical_bytes'] = max(peak['logical_bytes'], logical)
            peak['allocated_bytes'] = max(peak['allocated_bytes'], allocated)
            peak['samples'] += 1
        def watch():
            while not stop.wait(0.01):
                sample()
        worker = threading.Thread(target=watch)
        started = time.monotonic()
        worker.start()
        try:
            result = operation()
        finally:
            stop.set()
            worker.join()
            sample()
            records.append(dict(phase=label, wall_seconds=time.monotonic() - started, **peak))
        return result

    anchor = measured('cold-fixture-capture', lambda: native.collect_fixture(
        ROOT / 'driver/tools/quality/fixtures/rust-native/driver_complete.rs', directory / 'capture',
        RUNTIME / 'bin/harness-gate-rust-native-driver', RUNTIME / 'rust'))
    first = measured('first-full-reexport', lambda: native.certify(directory / 'capture', anchor))
    second = measured('warm-full-reexport', lambda: native.certify(directory / 'capture', anchor))
    test.assertEqual(first, second)
    write(directory / 'first.json', first)
    write(directory / 'second.json', second)
    write(directory / 'cost.json', {'scope': 'single-file-fixture; disk sampled at 10ms, hardlinks counted once; excludes immutable runtime',
                                  'anchor': anchor, 'measurements': records})


def relocation(test):
    """Copy actual installed payload bytes to a different executable path."""
    destination = ROOT / 'relocated-runtime'
    shutil.copytree(RUNTIME, destination, ignore=shutil.ignore_patterns('.delivery'))
    original = {str(p.relative_to(RUNTIME)): native.file_hash(p) for p in RUNTIME.rglob('*')
                if p.is_file() and '.delivery' not in p.relative_to(RUNTIME).parts}
    copied = {str(p.relative_to(destination)): native.file_hash(p) for p in destination.rglob('*') if p.is_file()}
    test.assertEqual(original, copied)
    output = ROOT / 'relocation-artifacts'
    output.mkdir()
    binding = json.loads((ROOT / 'configured/binding.json').read_text())
    binding['input']['output_root'] = str(output)
    request, _ = request_for(binding, ROOT / 'relocation-binding.json')
    request['adapter'] = json.loads((ROOT / 'configured/unsigned-request.json').read_text())['adapter']
    result = test.command('clean-physical-relocation',
        [str(destination / 'bin/harness-gate-rust-collector'), *request['args']], json.dumps(request))
    test.assertEqual(result.returncode, 1, result.stderr)
    test.assertIn('no relocation equivalence', json.loads(result.stdout)['collection']['error'])
    test.assertEqual(list(output.iterdir()), [])
    write(ROOT / 'relocation-receipt.json', {'payload_inventory_sha256': contract.fingerprint(copied),
        'files': len(copied), 'logical_bytes': sum(p.stat().st_size for p in destination.rglob('*') if p.is_file()),
        'original_runtime': str(RUNTIME), 'relocated_runtime': str(destination),
        'capture_anchor': binding['capture']['anchor'], 'result': 'rejected; no measurement-series equivalence'})


if __name__ == '__main__':
    test = setup()
    globals()[sys.argv[1]](test)
