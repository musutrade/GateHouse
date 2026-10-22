from __future__ import annotations

import copy
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/quality"))
import ci_quality
from source_measure import HOTSPOTS, SERIES, SOURCE_FILES, prepare, ast, byte_span, closure_name, compare, compiler_configuration, complexity, digest, instrument, measure, original_point, provenance

PROCESS_COMMAND_BASE_COMMIT = '3d887a460bb72064b29a6f9ea71293e2949a16d7'
PROCESS_COMMAND_BASE_SHA256 = '80118e08ebab122498013269a47a12c6b3dca80107ead21a3fbd81d4856aff61'
# Exact committed bytes: shallow CI checkouts do not require the old object.
PROCESS_COMMAND_BASE = r'''use std::process::{Child, Command, ExitStatus};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
pub(super) fn isolate_process_tree(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    unsafe {
        // SAFETY: `pre_exec` runs after fork and only invokes `setsid`, which is
        // async-signal-safe and does not touch Rust synchronization primitives.
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
}

#[cfg(not(unix))]
pub(super) fn isolate_process_tree(_command: &mut Command) {
    // Windows termination uses `taskkill /T` below to cover descendants. The
    // command itself has no portable process-group primitive to configure here.
}

#[cfg(unix)]
pub(super) fn terminate(child: &mut Child) -> std::io::Result<ExitStatus> {
    let process_group = -(child.id() as i32);
    // The child is its process-group leader, so this also stops spawned test/build processes.
    send_signal(process_group, libc::SIGTERM)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(50));
    }
    send_signal(process_group, libc::SIGKILL)?;
    child.wait()
}

#[cfg(unix)]
fn send_signal(process_group: i32, signal: libc::c_int) -> std::io::Result<()> {
    // SAFETY: the process group is created by `isolate_process_tree` and the
    // signal values are fixed constants owned by this module.
    let result = unsafe { libc::kill(process_group, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(not(unix))]
pub(super) fn terminate(child: &mut Child) -> std::io::Result<ExitStatus> {
    #[cfg(windows)]
    {
        let pid = child.id().to_string();
        let tree_status = Command::new("taskkill")
            .args(["/PID", &pid, "/T", "/F"])
            .status();
        if !tree_status.is_ok_and(|status| status.success()) {
            let _ = child.kill();
        }
    }
    #[cfg(not(windows))]
    child.kill()?;
    child.wait()
}
'''

CFG_TRY_SOURCE = '''fn checked(ok: bool) -> Result<(), ()> {
    if ok { Ok(()) } else { Err(()) }
}
pub fn configured(ok: bool) -> Result<u8, ()> {
    std::hint::black_box(ok);
    #[cfg(unix)]
    checked(
        ok
    )?;
    #[cfg(windows)]
    checked(
        ok
    )?;
    Ok(7)
}
'''


CFG_MATCH_ARM_SOURCE = '''pub fn configured(value: u8) -> u8 {
    match value {
        #[cfg(target_os = "linux")] 0 if value == 0 => { let hot = || if value == 0 { 7 } else { 8 }; hot() },
        #[cfg(target_os = "macos")] 0 if value == 0 => { let hot = || if value == 0 { 7 } else { 8 }; hot() },
        #[cfg(windows)] 0 if value == 0 => { let hot = || if value == 0 { 7 } else { 8 }; hot() },
        #[cfg(not(any(unix, windows)))] 9 if value == 9 => { let hidden = || 99; hidden() }, 1 => { let cold = || 9; cold() },
        _ => 3,
    }
}
pub fn unused() -> u8 { 11 }
'''


class SourceMeasureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE',
                                       ROOT / 'target/gh285-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='measurer-release-', dir=retained))
        if sys.platform != 'linux' or platform.machine() != 'x86_64':
            raise unittest.SkipTest(
                'the pinned measurer release supports Linux amd64 only; '
                'cross-platform analyzer builds belong to the plugin')
        cache = Path(os.environ.get('HARNESS_GATE_MEASURER_CACHE',
                                    ROOT / 'target/rust-measure-release')).resolve()
        pin = ci_quality.load_measurer_pin()
        cls.binary = ci_quality.ensure_measurer(cache)
        record = {'schema': 'rust-measure-release-record/v1', 'series': SERIES,
                  'release': {'tag': pin['release'], 'asset': pin['asset'], 'sha256': pin['sha256']},
                  'analyzer': {'name': pin['analyzer'], 'version': pin['version'], 'rule': pin['rule']},
                  'executable': str(cls.binary.resolve()),
                  'binary_sha256': digest(cls.binary.read_bytes()),
                  'rustc': subprocess.check_output(['rustc', '-vV'], text=True).strip()}
        cls.build_record = evidence / 'release-record.json'
        cls.build_record.write_text(json.dumps(record, indent=2) + '\n')

    def inventory(self, source, target=None):
        with tempfile.TemporaryDirectory() as temp:
            file = Path(temp) / "input.rs"
            file.write_bytes(source.encode())
            return ast(file, self.binary, compiler_configuration(target))

    def test_compiler_target_filters_modules_methods_fields_and_body_blocks(self):
        source = '''
struct State { #[cfg(windows)] handle: usize, #[cfg(unix)] fd: usize }
#[cfg(unix)] mod platform { fn open() { if true {} } }
#[cfg(windows)] mod platform { fn open() { if true {} if true {} } }
impl State { #[cfg(unix)] fn unix() {} #[cfg(windows)] fn windows() {} }
trait Run { #[cfg(unix)] fn unix() {} #[cfg(windows)] fn windows() {} }
#[cfg(not(test))]
fn both() {
    #[cfg(unix)] { if true {} }
    #[cfg(windows)] { if true {} if true {} let hidden = || if true {}; }
    #[cfg(test)] { unsupported!(a => b); }
}
#[cfg(test)] mod tests { #[test] fn ignored() { unsupported!(a => b); } }
#[cfg(all(test, unix))] mod external_tests;
#[test] fn ignored_function() { unsupported!(a => b); }
'''
        unix = self.inventory(source, 'x86_64-unknown-linux-gnu')
        windows = self.inventory(source, 'x86_64-pc-windows-msvc')
        self.assertIn('unix', unix['configuration']['cfg'])
        self.assertIn('windows', windows['configuration']['cfg'])
        self.assertEqual([s['name'] for s in unix['symbols']], ['platform::open', 'State::unix', 'unix', 'both'])
        self.assertEqual([complexity(s['raw']) for s in unix['symbols']], [2, 1, 1, 2])
        self.assertEqual([s['name'] for s in windows['symbols']], ['platform::open', 'State::windows', 'windows', 'both', 'both::closure_10_58'])
        self.assertEqual([complexity(s['raw']) for s in windows['symbols']], [3, 1, 1, 3, 2])
        for result in (unix, windows):
            self.assertTrue(result['excluded'])
            self.assertFalse(any(s['test'] for s in result['symbols']))

    def test_statement_cfg_attributes_on_unsafe_blocks_are_removable(self):
        source = '''pub fn configured() {
    #[cfg(unix)]
    unsafe { if true {} }
    #[cfg(windows)]
    unsafe { if true {} if true {} let hidden = || if true {}; }
}
'''
        unix = self.inventory(source, 'x86_64-unknown-linux-gnu')
        windows = self.inventory(source, 'x86_64-pc-windows-msvc')
        self.assertEqual([s['name'] for s in unix['symbols']], ['configured'])
        self.assertEqual([complexity(s['raw']) for s in unix['symbols']], [2])
        self.assertEqual([s['name'] for s in windows['symbols']],
                         ['configured', 'configured::closure_5_49'])
        self.assertEqual([complexity(s['raw']) for s in windows['symbols']], [3, 2])
        self.assertTrue(unix['excluded'])
        self.assertTrue(windows['excluded'])

    def test_unsupported_configuration_fails_without_partial_inventory(self):
        inactive = "unix" if "windows" in compiler_configuration()["cfg"] else "windows"
        for source in (
            '#[cfg(feature="extra")] fn f() {}',
            '#[cfg(any(unix, any(windows, test)))] fn f() {}',
            '#[cfg(target_os="android")] fn f() {}',
            '#[cfg(not(not(test)))] fn f() {}',
            '#[cfg(all(test, feature="extra"))] mod unsupported;',
            '#[cfg_attr(unix, inline)] fn f() {}',
            '#[cfg(windows)] #[cfg(feature="extra")] fn f() {}',
            'fn f() { let x = #[cfg(' + inactive + ')] { 1 }; }',
            'fn f() { println!("{}", #[cfg(unix)] { 1 }); }',
        ):
            with self.subTest(source=source), tempfile.TemporaryDirectory() as temp:
                file = Path(temp) / 'input.rs'; file.write_bytes(source.encode())
                result = subprocess.run([str(self.binary), str(file), '--target-cfg'],
                                        input=json.dumps(compiler_configuration()), text=True, capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('unsupported', result.stderr)
                self.assertEqual(result.stdout, '')

    def test_replay_and_project_sources_are_measured_for_the_host_target(self):
        for path in ('process/replay.rs', 'project/discovery.rs', 'project/mod.rs'):
            with self.subTest(path=path):
                self.assertIn(path, SOURCE_FILES)
                source = ROOT / 'tools/harness-gate/src' / path
                result = ast(source, self.binary)
                self.assertTrue(result['symbols'])
                self.assertFalse(any(s['test'] for s in result['symbols']))
                if path == 'process/replay.rs':
                    names = {s['name'] for s in result['symbols']}
                    host_is_windows = 'windows' in compiler_configuration()['cfg']
                    self.assertEqual('platform::open_at' in names, not host_is_windows)
                    self.assertEqual('platform::open_directory' in names, host_is_windows)
                    windows = ast(source, self.binary, compiler_configuration('x86_64-pc-windows-msvc'))
                    windows_names = {s['name'] for s in windows['symbols']}
                    self.assertIn('platform::open_directory', windows_names)
                    self.assertNotIn('platform::open_at', windows_names)

    def test_compare_rejects_mixed_target_configurations(self):
        base = {'series': SERIES, 'tools': {}, 'configuration': compiler_configuration(), 'functions': []}
        other = 'x86_64-unknown-linux-gnu' if 'windows' in base['configuration']['cfg'] else 'x86_64-pc-windows-msvc'
        head = dict(base, configuration=compiler_configuration(other))
        with self.assertRaisesRegex(ValueError, 'incompatible base/head target configurations'):
            compare(base, head)
        del base['configuration']; del head['configuration']
        with self.assertRaisesRegex(ValueError, 'incompatible base/head target configurations'):
            compare(base, head)

    def test_plan_sources_preserve_instrumentation_across_supported_target_configurations(self):
        paths = ('app/plan.rs', 'verify/inspection.rs', 'config/scope.rs', 'utils/git.rs', 'scope/mod.rs', 'plugins.rs', 'project/input.rs', 'config/impact.rs')
        for path in paths:
            self.assertIn(path, SOURCE_FILES)
            source = (ROOT / 'tools/harness-gate/src' / path).read_text(encoding='utf-8')
            for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'):
                with self.subTest(path=path, target=target):
                    inventory = self.inventory(source, target)
                    symbols = inventory['symbols']
                    self.assertFalse(any(s['test'] for s in symbols))
                    if path == 'scope/mod.rs':
                        self.assertEqual(symbols, [])
                    else:
                        self.assertTrue(symbols)
                    transformed, edits = instrument(source, inventory)
                    reparsed = self.inventory(transformed, target)
                    self.assertEqual([s['kind'] for s in reparsed['symbols']], [s['kind'] for s in symbols])
                    self.assertEqual([complexity(s['raw']) for s in reparsed['symbols']],
                                     [complexity(s['raw']) for s in symbols])
                    for original, inserted in zip(symbols, reparsed['symbols']):
                        mapped = byte_span(transformed, inserted['span'])
                        self.assertEqual((*original_point(mapped[:2], edits), *original_point(mapped[2:], edits)),
                                         byte_span(source, original['span']))

    def test_plan_selection_refuses_previous_or_mixed_series(self):
        previous = dict(SERIES, selection='gh315-review-low/1')
        for old, new in ((previous, SERIES), (SERIES, previous)):
            with self.assertRaisesRegex(ValueError, 'incompatible base/head series'):
                compare({'series': old}, {'series': new})

    def test_scope_detection_inventory_and_instrumentation(self):
        path = 'scope/detection.rs'
        self.assertIn(path, SOURCE_FILES)
        source = (ROOT / 'tools/harness-gate/src' / path).read_text(encoding='utf-8')
        for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'):
            with self.subTest(target=target):
                inventory = self.inventory(source, target)
                symbols = inventory['symbols']
                self.assertEqual([s['name'] for s in symbols if s['kind'] == 'function'],
                                 ['detect', 'detect_with_access', 'ensure_git_worktree'])
                self.assertFalse(any(s['test'] for s in symbols))
                transformed, edits = instrument(source, inventory)
                reparsed = self.inventory(transformed, target)
                self.assertEqual([s['name'] for s in reparsed['symbols']],
                                 [s['name'] for s in symbols])
                self.assertEqual([complexity(s['raw']) for s in reparsed['symbols']],
                                 [complexity(s['raw']) for s in symbols])
                self.assertTrue(all(s['kind'] != 'closure' for s in symbols))
                self.assertEqual(edits, [])
                self.assertEqual(transformed, source)

    def test_native_inventory_has_separate_identity_and_contracts(self):
        with tempfile.TemporaryDirectory() as temp:
            file = Path(temp) / "input.rs"
            file.write_text('fn f() { tracing::error!(value = %value); }')
            run = subprocess.run([str(self.binary), "--native-inventory", str(file)],
                                 text=True, capture_output=True, check=True)
            result = json.loads(run.stdout)
            self.assertEqual(result["analyzer"], "harness-gate-rust-native-inventory")
            self.assertFalse(result["certified_llvm_mapping"])
            with self.assertRaises(subprocess.CalledProcessError):
                ast(file, self.binary)

    def test_quality_configuration_source_certification(self):
        crate = ROOT / 'tools/harness-gate'
        for name in ('mod', 'model', 'policy', 'validation', 'compiler', 'collectors', 'baseline', 'baseline/git'):
            path = f'config/quality/{name}.rs'
            with self.subTest(path=path):
                self.assertIn(path, SOURCE_FILES)
                symbols = ast(crate / 'src' / path, self.binary)['symbols']
                production = [s for s in symbols if not s['test']]
                if name == 'model':
                    self.assertEqual(production, [])
                else:
                    self.assertTrue(production)
                    source = (crate / 'src' / path).read_text(encoding="utf-8")
                    transformed, _ = instrument(source, {'symbols': symbols})
                    self.assertEqual(len(self.inventory(transformed)['symbols']), len(symbols))
        module = (crate / 'src/config/quality/mod.rs').read_text(encoding="utf-8")
        self.assertIn('#[cfg(test)]\nmod tests;', module)

    def test_preset_source_certification(self):
        crate = ROOT / 'tools/harness-gate'
        inventory = json.loads((ROOT / 'tools/quality/production-source.json').read_text(encoding="utf-8"))
        declared = {path.removeprefix('src/') for path in inventory['boundaries']['preset']['files']}
        self.assertEqual(declared, {path for path in SOURCE_FILES if path.startswith('preset/')})
        for name in ('catalog', 'composition', 'filesystem', 'import', 'initialize', 'migration', 'mod'):
            path = f'preset/{name}.rs'
            with self.subTest(path=path):
                self.assertIn(path, SOURCE_FILES)
                source = crate / 'src' / path
                symbols = ast(source, self.binary)['symbols']
                if name != 'mod':
                    self.assertTrue(any(not symbol['test'] for symbol in symbols))
                transformed, _ = instrument(source.read_text(encoding="utf-8"), {'symbols': symbols})
                self.assertEqual(len(self.inventory(transformed)['symbols']), len(symbols))
        self.assertIn('#[cfg(test)]\nmod tests;', (crate / 'src/preset/mod.rs').read_text(encoding="utf-8"))

    def test_resource_validation_source_certification(self):
        path = 'config/validation/mod.rs'
        self.assertIn(path, SOURCE_FILES)
        symbols = ast(ROOT / 'tools/harness-gate/src' / path, self.binary)['symbols']
        names = {symbol['name'] for symbol in symbols if not symbol['test']}
        self.assertTrue({'validate_resource_conflicts', 'validate_shared_services',
                         'validate_log_conflicts', 'validate_service_injections'} <= names)
        for module in ('config', 'verify'):
            source = (ROOT / f'tools/harness-gate/src/{module}/mod.rs').read_text(encoding="utf-8")
            self.assertIn('#[cfg(test)]\nmod tests;', source)

    def test_automatic_history_archive_source_certification(self):
        inventory = json.loads((ROOT / 'tools/quality/production-source.json').read_text(encoding="utf-8"))
        declared = {path for boundary in inventory['boundaries'].values() for path in boundary['files']}
        for path in ('report_history/archive.rs', 'report_history/archive/filesystem.rs'):
            with self.subTest(path=path):
                self.assertIn(path, SOURCE_FILES)
                self.assertIn('src/' + path, declared)
                source = ROOT / 'tools/harness-gate/src' / path
                symbols = ast(source, self.binary)['symbols']
                self.assertTrue(any(not symbol['test'] for symbol in symbols))
                transformed, _ = instrument(source.read_text(encoding="utf-8"), {'symbols': symbols})
                self.assertEqual(len(self.inventory(transformed)['symbols']), len(symbols))

    def test_hotspot_source_certification(self):
        path = 'verify/report/quality_view/hotspots.rs'
        self.assertIn(path, SOURCE_FILES)
        inventory = json.loads((ROOT / 'tools/quality/production-source.json').read_text(encoding="utf-8"))
        self.assertIn('src/' + path, inventory['boundaries']['verify']['files'])
        source = ROOT / 'tools/harness-gate/src' / path
        symbols = ast(source, self.binary)['symbols']
        self.assertTrue(any(not symbol['test'] for symbol in symbols))
        transformed, _ = instrument(source.read_text(encoding="utf-8"), {'symbols': symbols})
        self.assertEqual(len(self.inventory(transformed)['symbols']), len(symbols))

    def test_dashboard_source_certification(self):
        inventory = json.loads((ROOT / 'tools/quality/production-source.json').read_text(encoding="utf-8"))
        declared = {path for boundary in inventory['boundaries'].values() for path in boundary['files']}
        for path in ('report_history/dashboard.rs', 'report_history/dashboard/impact.rs'):
            with self.subTest(path=path):
                self.assertIn(path, SOURCE_FILES)
                self.assertIn('src/' + path, declared)
                source = ROOT / 'tools/harness-gate/src' / path
                symbols = ast(source, self.binary)['symbols']
                self.assertTrue(any(not symbol['test'] for symbol in symbols))
                transformed, _ = instrument(source.read_text(encoding="utf-8"), {'symbols': symbols})
                self.assertEqual(len(self.inventory(transformed)['symbols']), len(symbols))

    def test_verify_reporting_source_certification(self):
        crate = ROOT / 'tools/harness-gate'
        for path in ('verify/mod.rs', 'verify/quality.rs', 'verify/report.rs', 'failure.rs', 'config/import.rs'):
            with self.subTest(path=path):
                self.assertIn(path, SOURCE_FILES)
                source = crate / 'src' / path
                symbols = ast(source, self.binary)['symbols']
                self.assertTrue(any(not symbol['test'] for symbol in symbols))
                transformed, _ = instrument(source.read_text(encoding="utf-8"), {'symbols': symbols})
                self.assertEqual(len(self.inventory(transformed)['symbols']), len(symbols))

    def test_report_retention_sources_inventory_and_instrumentation(self):
        paths = {'service/lease.rs', 'service/lease/container.rs', 'service/mod.rs', 'service/report_directory.rs', 'service/legacy_cli.rs', 'service/runtime.rs', 'service/docker.rs',
                 'service/lifecycle.rs', 'service/lifecycle/recovery.rs',
                 'service/lifecycle_public.rs', 'service/lifecycle_public/linux.rs',
                 'service/lifecycle_public/unsupported.rs'}
        self.assertEqual({path for path in SOURCE_FILES if path.startswith('service/')}, paths)
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh286-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='service-three-target-', dir=retained))
        for target, os_name in (('x86_64-unknown-linux-gnu', 'linux'),
                                ('aarch64-apple-darwin', 'macos'),
                                ('x86_64-pc-windows-msvc', 'windows')):
            for path in sorted(paths):
                with self.subTest(target=target, path=path):
                    source_file = ROOT / 'tools/harness-gate/src' / path
                    source = source_file.read_text(encoding='utf-8')
                    inventory = ast(source_file, self.binary, compiler_configuration(target))
                    symbols = inventory['symbols']
                    if path == 'service/lifecycle_public.rs':
                        # This routing module contains only a data struct and cfg module/reexports.
                        self.assertEqual(symbols, [])
                        self.assertIn('pub(crate) struct Config', source)
                        self.assertIn('mod linux;', source)
                        self.assertIn('mod unsupported;', source)
                    else:
                        self.assertTrue(any(not symbol['test'] for symbol in symbols))
                    self.assertFalse(any(symbol['test'] for symbol in symbols))
                    if path == 'service/lease.rs':
                        variants = [symbol for symbol in symbols if symbol['name'] == 'process_start_identity_checked']
                        self.assertEqual(len(variants), 1)
                        self.assertIn(os_name + ':', variants[0]['syntax'])
                        self.assertTrue(inventory['excluded'])
                    elif path == 'service/report_directory.rs':
                        exclusions = [self.cfg_source_slice(source, span) for span in inventory['excluded']]
                        windows_arm = '#[cfg(windows)]\n            Err(error) if matches!(error.raw_os_error(), Some(32 | 33)) => return Ok(None),'
                        self.assertEqual(windows_arm in exclusions, os_name != 'windows')
                        self.assertTrue(any('not(any(unix, windows))' in fragment for fragment in exclusions))
                    transformed, edits = instrument(source, inventory)
                    reparsed = self.inventory(transformed, target)
                    self.assertEqual([symbol['kind'] for symbol in reparsed['symbols']],
                                     [symbol['kind'] for symbol in symbols])
                    self.assertEqual([symbol['name'] for symbol in reparsed['symbols'] if symbol['kind'] == 'function'],
                                     [symbol['name'] for symbol in symbols if symbol['kind'] == 'function'])
                    self.assertEqual([symbol['raw'] for symbol in reparsed['symbols']],
                                     [symbol['raw'] for symbol in symbols])
                    # Map transformed exclusion endpoints back to original byte
                    # coordinates; closure insertion must preserve full ranges.
                    original_ranges = [byte_span(source, span) for span in inventory['excluded']]
                    remapped_ranges = [(*original_point(byte_span(transformed, span)[:2], edits),
                                        *original_point(byte_span(transformed, span)[2:], edits))
                                       for span in reparsed['excluded']]
                    self.assertEqual(remapped_ranges, original_ranges)
                    case = evidence / target / path
                    case.parent.mkdir(parents=True, exist_ok=True)
                    case.write_text(json.dumps({'source_sha256': digest(source.encode()),
                        'inventory': inventory, 'reparsed': reparsed, 'edits': edits,
                        'analyzer_build_record': str(self.build_record)}, indent=2) + '\n')

    @staticmethod
    def cfg_source_slice(source, span):
        points = byte_span(source, span)
        lines = source.encode().splitlines(keepends=True)
        start = sum(map(len, lines[:points[0] - 1])) + points[1] - 1
        end = sum(map(len, lines[:points[2] - 1])) + points[3] - 1
        return source.encode()[start:end].decode()

    def test_process_reader_source_inventory_and_instrumentation(self):
        path = 'process/reader.rs'
        self.assertIn(path, SOURCE_FILES)
        source_file = ROOT / 'tools/harness-gate/src' / path
        source = source_file.read_bytes().decode('utf-8')
        for target, unix in (('x86_64-unknown-linux-gnu', True), ('aarch64-apple-darwin', True),
                             ('x86_64-pc-windows-msvc', False)):
            with self.subTest(target=target):
                inventory = ast(source_file, self.binary, compiler_configuration(target))
                symbols = inventory['symbols']
                functions = [s['name'] for s in symbols if s['kind'] == 'function']
                for name in ('spawn_limited_reader', 'collect_limited_reader', 'read_limited'):
                    self.assertEqual(functions.count(name), 1)
                # Exactly one platform variant of the stop-aware wait is measured.
                waits = [s for s in symbols if s['name'] == 'wait_readable']
                self.assertEqual(len(waits), 1)
                self.assertEqual('poll' in waits[0]['syntax'], unix)
                self.assertFalse(any(s['test'] for s in symbols))
                self.assertTrue(inventory['excluded'], 'platform and test items leave production ranges')
                transformed, edits = instrument(source, inventory)
                reparsed = self.inventory(transformed, target)
                self.assertEqual([s['kind'] for s in reparsed['symbols']], [s['kind'] for s in symbols])
                for original, inserted in zip(symbols, reparsed['symbols']):
                    self.assertEqual(complexity(original['raw']), complexity(inserted['raw']))
                    for field in ('span', 'body'):
                        mapped = byte_span(transformed, inserted[field])
                        self.assertEqual((*original_point(mapped[:2], edits),
                                          *original_point(mapped[2:], edits)),
                                         byte_span(source, original[field]))
    def test_review_low_sources_inventory_and_instrumentation(self):
        paths = ('audit/runner.rs', 'config/loader.rs', 'config/migration.rs', 'config/model.rs',
                 'process/signal.rs', 'secrets/config.rs')
        for path in paths:
            self.assertIn(path, SOURCE_FILES)
            source_file = ROOT / 'tools/harness-gate/src' / path
            source = source_file.read_bytes().decode('utf-8')
            for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'):
                with self.subTest(path=path, target=target):
                    inventory = ast(source_file, self.binary, compiler_configuration(target))
                    symbols = inventory['symbols']
                    self.assertTrue(any(s['kind'] == 'function' for s in symbols))
                    self.assertFalse(any(s['test'] for s in symbols))
                    transformed, edits = instrument(source, inventory)
                    reparsed = self.inventory(transformed, target)
                    self.assertEqual([s['kind'] for s in reparsed['symbols']], [s['kind'] for s in symbols])
                    for original, inserted in zip(symbols, reparsed['symbols']):
                        self.assertEqual(complexity(original['raw']), complexity(inserted['raw']))
                        for field in ('span', 'body'):
                            mapped = byte_span(transformed, inserted[field])
                            self.assertEqual((*original_point(mapped[:2], edits),
                                              *original_point(mapped[2:], edits)),
                                             byte_span(source, original[field]))

    def test_retired_net_policy_is_not_in_the_production_inventory(self):
        # Issue #129 retired webhook delivery; the unused net_policy module was
        # deleted from both the crate and the measured production set.
        self.assertNotIn('net_policy.rs', SOURCE_FILES)
        inventory = json.loads((ROOT / 'tools/quality/production-source.json').read_text(encoding='utf-8'))
        self.assertNotIn('net_policy', inventory['boundaries'])
        self.assertFalse((ROOT / 'tools/harness-gate/src/net_policy.rs').exists())
        self.assertNotIn('mod net_policy;', (ROOT / 'tools/harness-gate/src/main.rs').read_text(encoding='utf-8'))

    def test_actual_redaction_source_inventory_and_instrumentation(self):
        self.assertEqual(SERIES, {
            'analyzer': 'harness-gate-rust-measure/0.3.1', 'rule': 'mccabe-rust-3/1',
            'instrumentation': 'closure-black-box/1', 'mapping': 'insertions-utf8/1',
            'selection': 'gh33-findings/1', 'configuration': 'compiler-target-production/4',
        })
        path = 'utils/redaction.rs'
        self.assertIn(path, SOURCE_FILES)
        source_file = ROOT / 'tools/harness-gate/src' / path
        source = source_file.read_bytes().decode('utf-8')
        inventory = ast(source_file, self.binary)
        self.assertEqual(inventory, self.inventory(source))
        symbols = inventory['symbols']
        self.assertEqual([s['kind'] for s in symbols], ['function', 'function', 'closure'])
        self.assertEqual([s['name'] for s in symbols],
                         ['redact_text', 'redact_text_cow', 'redact_text_cow::closure_18_41'])
        self.assertFalse(any(s['test'] for s in symbols))
        self.assertEqual([complexity(s['raw']) for s in symbols], [1, 4, 1])
        self.assertTrue(inventory['excluded'], 'test module must leave production ranges')
        transformed, edits = instrument(source, inventory)
        self.assertEqual(len(edits), 2)
        reparsed = self.inventory(transformed)
        self.assertEqual([s['kind'] for s in reparsed['symbols']], [s['kind'] for s in symbols])
        for original, inserted in zip(symbols, reparsed['symbols']):
            self.assertEqual(complexity(original['raw']), complexity(inserted['raw']))
            for field in ('span', 'body'):
                mapped = byte_span(transformed, inserted[field])
                self.assertEqual((*original_point(mapped[:2], edits),
                                  *original_point(mapped[2:], edits)),
                                 byte_span(source, original[field]))

    def test_once_lock_regex_tuple_fold_has_independent_native_owners(self):
        source = '''use regex::Regex;
use std::sync::OnceLock;
pub fn redact(input: &str, patterns: &OnceLock<Vec<(Regex, &'static str)>>, count: usize) -> String {
    let patterns = patterns.get_or_init(|| {
        vec![
            (Regex::new("SYNTHETIC_A").expect("first regex"), "public-a"),
            (Regex::new("SYNTHETIC_B").expect("second regex"), "public-b"),
        ].into_iter().take(count).collect()
    });
    patterns.iter().fold(input.to_string(), |text, (pattern, replacement)| {
        pattern.replace_all(&text, *replacement).into_owned()
    })
}
#[cfg(test)] mod tests { #[test] fn excluded() { unsupported!(a => b); } }
'''
        main = '''mod redaction;
use regex::Regex;
use std::sync::OnceLock;
fn main() {
    let mode = std::env::args().nth(1).expect("mode");
    let count = if mode.ends_with("zero") { 0 } else { 2 };
    let patterns = OnceLock::new();
    if mode.starts_with("preset") {
        patterns.set(vec![
            (Regex::new("SYNTHETIC_A").unwrap(), "public-a"),
            (Regex::new("SYNTHETIC_B").unwrap(), "public-b"),
        ].into_iter().take(count).collect()).unwrap();
    }
    let result = redaction::redact("public-context SYNTHETIC_A SYNTHETIC_B", &patterns, count);
    assert_eq!(result, if count == 0 { "public-context SYNTHETIC_A SYNTHETIC_B" }
                       else { "public-context public-a public-b" });
}
'''
        # This fixture builds its own pinned regex dependency graph. Never join
        # against an unbound rlib left by another build in a shared target dir.
        repo_lock = (ROOT / 'tools/harness-gate/Cargo.lock').read_bytes()
        packages = tomllib.loads(repo_lock.decode())['package']
        by_name = {}
        for package in packages:
            by_name.setdefault(package['name'], []).append(package)
        selected = {}
        pending = ['regex']
        while pending:
            name = pending.pop()
            if name in selected:
                continue
            self.assertEqual(len(by_name[name]), 1, f'ambiguous fixture dependency: {name}')
            selected[name] = by_name[name][0]
            pending.extend(dep.split()[0] for dep in selected[name].get('dependencies', []))
        regex_version = selected['regex']['version']
        cargo_manifest = ('[package]\nname = "redaction-native-fixture"\nversion = "0.0.0"\n'
                          'edition = "2021"\n[workspace]\n[dependencies]\n'
                          f'regex = "={regex_version}"\n')
        lock = ('version = 4\n\n[[package]]\nname = "redaction-native-fixture"\n'
                'version = "0.0.0"\ndependencies = ["regex"]\n\n')
        for block in repo_lock.decode().split('[[package]]\n')[1:]:
            package = tomllib.loads('[[package]]\n' + block)['package'][0]
            if package['name'] in selected:
                self.assertEqual(package, selected[package['name']])
                lock += '[[package]]\n' + block
        lock_bytes = lock.encode()
        retained_root = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE',
                                            ROOT / 'target/gh287-native-evidence'))
        retained_root.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='once-lock-fold-', dir=retained_root))
        commands = []

        def run(command, **kwargs):
            record = {'argv': [str(arg) for arg in command], 'cwd': str(kwargs.get('cwd', ROOT)),
                      'environment': kwargs.pop('record_environment', {})}
            if 'input' in kwargs:
                record['stdin_sha256'] = digest(kwargs['input'])
            commands.append(record)
            number = len(commands)
            try:
                result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwargs)
            except OSError as error:
                record['error'] = str(error)
                (evidence / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
                raise
            record['returncode'] = result.returncode
            (evidence / f'command-{number}.stdout').write_bytes(result.stdout)
            (evidence / f'command-{number}.stderr').write_bytes(result.stderr)
            (evidence / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
            self.assertEqual(result.returncode, 0, f'{command}: {result.stderr.decode(errors="replace")}')
            return result.stdout

        with tempfile.TemporaryDirectory(dir=ROOT / 'target') as temp:
            crate = Path(temp)
            (crate / 'src').mkdir()
            file = crate / 'src/redaction.rs'
            file.write_bytes(source.encode())
            configuration = compiler_configuration()
            inventory = json.loads(run([str(self.binary), str(file), '--target-cfg'],
                                       input=json.dumps(configuration).encode()))
            self.assertEqual(inventory['configuration'], configuration)
            self.assertEqual([s['kind'] for s in inventory['symbols']], ['function', 'closure', 'closure'])
            self.assertEqual([complexity(s['raw']) for s in inventory['symbols']], [1, 1, 1])
            transformed, edits = instrument(source, inventory)
            file.write_bytes(transformed.encode())
            (crate / 'src/main.rs').write_bytes(main.encode())
            (crate / 'Cargo.toml').write_bytes(cargo_manifest.encode())
            (crate / 'Cargo.lock').write_bytes(lock_bytes)
            manifest = {'series': SERIES, 'configuration': configuration, 'files': {'redaction.rs': {
                'original': source, 'original_sha256': digest(source.encode()),
                'instrumented_sha256': digest(transformed.encode()), 'inventory': inventory, 'edits': edits}}}
            for name, data in [('fixture.original.rs', source.encode()), ('fixture.instrumented.rs', transformed.encode()),
                               ('main.rs', main.encode()), ('Cargo.toml', cargo_manifest.encode()), ('Cargo.lock', lock_bytes),
                               ('repository.Cargo.lock', repo_lock)]:
                (evidence / name).write_bytes(data)
            (evidence / 'instrumentation-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
            environment = {**os.environ, 'CARGO_TARGET_DIR': str(crate / 'build'),
                           'RUSTFLAGS': '-C instrument-coverage -C opt-level=0'}
            environment.pop('CARGO_ENCODED_RUSTFLAGS', None)
            environment.pop('LLVM_PROFILE_FILE', None)
            run(['cargo', 'fetch', '--locked', '--manifest-path', str(crate / 'Cargo.toml'),
                 '--target', manifest['configuration']['target']], cwd=crate, env=environment)
            self.assertEqual((crate / 'Cargo.lock').read_bytes(), lock_bytes)
            run(['cargo', 'build', '--locked', '--offline', '--manifest-path', str(crate / 'Cargo.toml'),
                 '--target', manifest['configuration']['target']], cwd=crate, env=environment,
                record_environment={key: environment[key] for key in ('CARGO_TARGET_DIR', 'RUSTFLAGS')})
            self.assertEqual((crate / 'Cargo.lock').read_bytes(), lock_bytes)
            suffix = '.exe' if os.name == 'nt' else ''
            executable = crate / 'build' / manifest['configuration']['target'] / 'debug' / ('redaction-native-fixture' + suffix)
            shutil.copyfile(executable, evidence / executable.name)
            sysroot = run(['rustc', '--print', 'sysroot']).decode().strip()
            version = run(['rustc', '-vV']).decode().strip()
            host = next(line.removeprefix('host: ') for line in version.splitlines() if line.startswith('host: '))
            llvm_bin = Path(sysroot) / 'lib/rustlib' / host / 'bin'
            llvm_tools = {name: llvm_bin / (name + suffix) for name in ('llvm-profdata', 'llvm-cov')}
            tool_versions = {name: run([str(path), '--version']).decode().strip() for name, path in llvm_tools.items()}
            results, rejected = {}, []
            for mode in ('fresh-zero', 'fresh-multi', 'preset-zero', 'preset-multi'):
                raw, profile = evidence / f'{mode}.profraw', evidence / f'{mode}.profdata'
                run([str(executable), mode], env={**environment, 'LLVM_PROFILE_FILE': str(raw)},
                    record_environment={'LLVM_PROFILE_FILE': str(raw)})
                run([str(llvm_tools['llvm-profdata']), 'merge', '-sparse', str(raw), '-o', str(profile)])
                exported = run([str(llvm_tools['llvm-cov']), 'export', str(executable), f'-instr-profile={profile}'])
                (evidence / f'{mode}.llvm.json').write_bytes(exported)
                llvm = json.loads(exported)
                rows = measure(manifest, llvm, crate, self.binary)
                rows_with_hits = measure(manifest, llvm, crate, self.binary, include_line_hits=True)
                self.assertEqual(len(rows), 3)
                for plain, detailed in zip(rows, rows_with_hits):
                    hits = dict(detailed.pop('line_hits'))
                    self.assertEqual(detailed, plain)
                    self.assertEqual(plain['lines']['count'], len(hits))
                    self.assertEqual(plain['lines']['covered'], sum(count > 0 for count in hits.values()))
                parent, initializer, fold = rows
                self.assertEqual([r['kind'] for r in rows], ['function', 'closure', 'closure'])
                expected = [1, int(mode.startswith('fresh')), 0 if mode.endswith('zero') else 2]
                for row, count in zip(rows, expected):
                    self.assertEqual(len(row['instances']), 1)
                    self.assertEqual(row['instances'][0]['count'], count, (mode, row['name']))
                    self.assertGreater(row['lines']['count'], 0)
                    self.assertGreater(row['regions']['count'], 0)
                    self.assertEqual(row['cc'], 1)
                    if count == 0:
                        self.assertEqual(row['lines']['covered'], 0)
                        self.assertEqual(row['regions']['covered'], 0)
                    else:
                        self.assertEqual(row['lines']['covered'], row['lines']['count'])
                        self.assertEqual(row['regions']['covered'], row['regions']['count'])
                self.assertEqual(len({r['instances'][0]['index'] for r in rows}), 3)
                results[mode] = rows
                for owner, row in zip(('parent', 'initializer', 'fold'), rows):
                    missing = copy.deepcopy(llvm)
                    missing['data'][0]['functions'].pop(row['instances'][0]['index'])
                    with self.assertRaisesRegex(ValueError, re.escape('missing LLVM function: redaction.rs:' + row['name'] + ':')):
                        measure(manifest, missing, crate, self.binary)
                    rejected.append(f'{mode}: missing {owner} (count={row["instances"][0]["count"]})')
                for owner, row in (('initializer', initializer), ('fold', fold)):
                    borrowed = copy.deepcopy(llvm)
                    function = borrowed['data'][0]['functions'][row['instances'][0]['index']]
                    native_parent = llvm['data'][0]['functions'][parent['instances'][0]['index']]
                    function['regions'] = copy.deepcopy(native_parent['regions'])
                    function['filenames'] = copy.deepcopy(native_parent['filenames'])
                    with self.assertRaisesRegex(ValueError, 'function kind mismatch: redaction.rs:redact'):
                        measure(manifest, borrowed, crate, self.binary)
                    rejected.append(f'{mode}: {owner} cannot borrow parent mapping')
                (evidence / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
                (evidence / 'negative-checks.json').write_text(json.dumps(rejected, indent=2) + '\n')
            report = {'series': SERIES, 'configuration': manifest['configuration'], 'tools': provenance(self.binary),
                      'checkout_sha': run(['git', 'rev-parse', 'HEAD'], cwd=ROOT).decode().strip(),
                      'fixture_binary_sha256': digest(executable.read_bytes()),
                      'test_source_sha256': digest(Path(__file__).read_bytes()),
                      'repository_lock_sha256': digest(repo_lock), 'fixture_lock_sha256': digest(lock_bytes),
                      'locked_packages': selected,
                      'llvm_tools': {name: {'sha256': digest(path.read_bytes()), 'version': tool_versions[name]}
                                     for name, path in llvm_tools.items()},
                      'results': results, 'validated': rejected}
            (evidence / 'native-result.json').write_text(json.dumps(report, indent=2) + '\n')
            hashes = {path.name: digest(path.read_bytes()) for path in sorted(evidence.iterdir())}
            (evidence / 'sha256.json').write_text(json.dumps(hashes, indent=2) + '\n')

    def process_command_sources(self):
        base = PROCESS_COMMAND_BASE.encode()
        self.assertEqual(digest(base), PROCESS_COMMAND_BASE_SHA256, 'base snapshot source hash mismatch')
        reference = PROCESS_COMMAND_BASE_COMMIT + ':tools/harness-gate/src/process/command.rs'
        # --batch-check returns a precise missing-object row for shallow clones;
        # Git failures or malformed responses are not converted into fallback.
        check = subprocess.run(['git', 'cat-file', '--batch-check'], cwd=ROOT,
                               input=reference + '\n', capture_output=True, text=True)
        self.assertEqual(check.returncode, 0, check.stderr)
        self.assertEqual(check.stderr, '')
        if check.stdout != reference + ' missing\n':
            self.assertRegex(check.stdout, r'^[0-9a-f]{40,64} blob [0-9]+\n$')
            self.assertEqual(subprocess.check_output(['git', 'show', reference], cwd=ROOT), base)
        head = (ROOT / 'tools/harness-gate/src/process/command.rs').read_bytes()
        return {'base': base.decode(), 'head': head.decode()}

    def test_actual_process_command_base_head_inventory_and_reparse(self):
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh285-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='process-command-ast-', dir=retained))
        self.assertIn('process/command.rs', SOURCE_FILES)
        self.assertEqual(SERIES, {
            'analyzer': 'harness-gate-rust-measure/0.3.1', 'rule': 'mccabe-rust-3/1',
            'instrumentation': 'closure-black-box/1', 'mapping': 'insertions-utf8/1',
            'selection': 'gh33-findings/1', 'configuration': 'compiler-target-production/4',
        })
        for label, source in self.process_command_sources().items():
            for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'):
                with self.subTest(source=label, target=target):
                    inventory = self.inventory(source, target)
                    case = evidence / label / target
                    case.mkdir(parents=True)
                    (case / 'original.rs').write_bytes(source.encode('utf-8'))
                    (case / 'inventory.json').write_text(json.dumps(inventory, indent=2) + '\n')
                    symbols = inventory['symbols']
                    windows = 'windows' in inventory['configuration']['cfg']
                    functions = [s['name'] for s in symbols if s['kind'] == 'function']
                    expected = (['isolate_process_tree', 'try_complete', 'terminate'] if label != 'base'
                                else ['isolate_process_tree', 'terminate']) if windows else (
                        ['isolate_process_tree', 'terminate', 'send_signal'] if label == 'base' else
                        ['isolate_process_tree', 'terminate', 'try_complete', 'terminate_with_signal', 'reap_timeout', 'observe_exit', 'observe_pid', 'send_signal']
                        + (['list_process_group', 'darwin_group_is_finished'] if 'target_os="macos"' in inventory['configuration']['cfg'] else []))
                    self.assertEqual(functions, expected)
                    closures = [s for s in symbols if s['kind'] == 'closure']
                    self.assertEqual(len(closures), 1)
                    self.assertTrue(closures[0]['name'].startswith('terminate::' if windows else 'isolate_process_tree::'))
                    self.assertEqual(complexity(closures[0]['raw']), 1 if windows else 2)
                    self.assertFalse(any(s['test'] for s in symbols))
                    self.assertTrue(inventory['excluded'])
                    transformed, edits = instrument(source, inventory)
                    self.assertEqual(len(edits), 2)
                    reparsed = self.inventory(transformed, target)
                    (case / 'instrumented.rs').write_bytes(transformed.encode('utf-8'))
                    (case / 'reparsed.json').write_text(json.dumps(reparsed, indent=2) + '\n')
                    (case / 'edits.json').write_text(json.dumps(edits, indent=2) + '\n')
                    self.assertEqual([(s['name'].split('::closure_')[0], s['kind'], s['raw']) for s in symbols],
                                     [(s['name'].split('::closure_')[0], s['kind'], s['raw']) for s in reparsed['symbols']])
                    for original, inserted in zip(symbols, reparsed['symbols']):
                        for field in ('span', 'body'):
                            mapped = byte_span(transformed, inserted[field])
                            self.assertEqual((*original_point(mapped[:2], edits), *original_point(mapped[2:], edits)),
                                             byte_span(source, original[field]))
        (evidence / 'validated.json').write_text(json.dumps({'series': SERIES,
            'base_commit': PROCESS_COMMAND_BASE_COMMIT, 'base_source_sha256': PROCESS_COMMAND_BASE_SHA256,
            'test_source_sha256': digest(Path(__file__).read_bytes()),
            'analyzer_build_record': {'path': str(self.build_record), 'sha256': digest(self.build_record.read_bytes())}},
            indent=2) + '\n')

    def test_process_command_base_snapshot_missing_object_and_tamper(self):
        from unittest.mock import patch
        reference = PROCESS_COMMAND_BASE_COMMIT + ':tools/harness-gate/src/process/command.rs'
        missing = subprocess.CompletedProcess([], 0, reference + ' missing\n', '')
        with patch.object(subprocess, 'run', return_value=missing), \
                patch.object(subprocess, 'check_output', side_effect=AssertionError('must not git-show a missing object')):
            sources = self.process_command_sources()
            self.assertEqual(digest(sources['base'].encode()), PROCESS_COMMAND_BASE_SHA256)
            self.assertNotEqual(sources['base'], sources['head'])
        with patch.dict(globals(), PROCESS_COMMAND_BASE=PROCESS_COMMAND_BASE + '// tamper\n'):
            with self.assertRaisesRegex(AssertionError, 'base snapshot source hash mismatch'):
                self.process_command_sources()
        failed = subprocess.CompletedProcess([], 128, '', 'repository unavailable')
        with patch.object(subprocess, 'run', return_value=failed):
            with self.assertRaisesRegex(AssertionError, 'repository unavailable'):
                self.process_command_sources()

    def test_cfg_macos_predicate_is_bounded_and_ranges_reparse(self):
        source = '''#[cfg(target_os = "macos")]
fn darwin() -> u8 { 7 }
fn common() -> u8 {
    #[cfg(target_os = "macos")]
    { return darwin(); }
    3
}
'''
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh285-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='cfg-macos-ast-', dir=retained))
        (evidence / 'source.rs').write_bytes(source.encode('utf-8'))
        results = {}
        for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'):
            configuration = compiler_configuration(target)
            darwin = 'target_os="macos"' in configuration['cfg']
            inventory = self.inventory(source, target)
            self.assertEqual([row['name'] for row in inventory['symbols']],
                             ['darwin', 'common'] if darwin else ['common'])
            expected_excluded = [] if darwin else [
                '#[cfg(target_os = "macos")]\nfn darwin() -> u8 { 7 }',
                '#[cfg(target_os = "macos")]\n    { return darwin(); }',
            ]
            raw = source.encode('utf-8')
            actual_excluded = []
            for span in inventory['excluded']:
                points = byte_span(source, span)
                lines = raw.splitlines(keepends=True)
                start = sum(map(len, lines[:points[0] - 1])) + points[1] - 1
                end = sum(map(len, lines[:points[2] - 1])) + points[3] - 1
                actual_excluded.append(raw[start:end].decode('utf-8'))
            self.assertEqual(actual_excluded, expected_excluded)
            transformed, edits = instrument(source, inventory)
            self.assertEqual(edits, [])
            self.assertEqual(self.inventory(transformed, target), inventory)
            refusals = {}
            for predicate in ('target_os="ios"',
                              'target_os=7', 'target_arch="aarch64"',
                              'not(target_os="macos")', 'all(unix, target_os="macos")'):
                file = evidence / (target + '-' + str(len(refusals)) + '.rs')
                file.write_bytes(('#[cfg(' + predicate + ')] fn f() {}\n').encode('utf-8'))
                result = subprocess.run([str(self.binary), str(file), '--target-cfg'],
                    input=json.dumps(configuration).encode(), capture_output=True, timeout=30)
                (file.with_suffix('.stdout')).write_bytes(result.stdout)
                (file.with_suffix('.stderr')).write_bytes(result.stderr)
                self.assertNotEqual(result.returncode, 0, predicate)
                self.assertEqual(result.stdout, b'')
                self.assertIn(b'unsupported cfg', result.stderr)
                refusals[predicate] = {'returncode': result.returncode,
                                      'stderr': result.stderr.decode('utf-8')}
            results[target] = {'configuration': configuration, 'inventory': inventory,
                               'excluded_bytes': actual_excluded, 'refusals': refusals}
            (evidence / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
        (evidence / 'validated.json').write_text(json.dumps({'series': SERIES,
            'source_sha256': digest(raw), 'test_source_sha256': digest(Path(__file__).read_bytes()),
            'analyzer_build_record': str(self.build_record), 'results': results}, indent=2) + '\n')

    def test_configuration4_rejects_old_series_before_any_payload(self):
        from unittest.mock import patch
        for old in ('compiler-target-production/1', 'compiler-target-production/2', 'compiler-target-production/3'):
            with self.subTest(old=old), patch('source_measure.compiler_configuration',
                                             side_effect=AssertionError('must reject before compiler lookup')):
                stale = dict(SERIES, configuration=old)
                with self.assertRaisesRegex(ValueError, 'incompatible measurement series'):
                    measure({'series': stale}, None, None, None)
                for base, head in (({'series': stale}, {'series': SERIES}),
                                   ({'series': SERIES}, {'series': stale}),
                                   ({'series': stale}, {'series': stale})):
                    with self.assertRaisesRegex(ValueError, 'incompatible base/head series'):
                        compare(base, head)

    def test_cfg_os_any_predicates_are_bounded_without_hidden_operands(self):
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh286-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='bounded-os-any-', dir=retained))
        rejected = ('target_os="ios"', 'target_os=7', 'target_arch="x86_64"',
                    'any(unix, feature="extra")', 'any(windows, feature="extra")',
                    'not(any(test, feature="extra"))', 'all(test, feature="extra")',
                    'any(unix, any(windows, test))', 'not(any(unix, not(test)))',
                    'not(not(test))', 'not(target_os="macos")', 'all(unix, target_os="macos")')
        results = {}
        for target, os_name in (('x86_64-unknown-linux-gnu', 'linux'),
                                ('aarch64-apple-darwin', 'macos'),
                                ('x86_64-pc-windows-msvc', 'windows')):
            configuration = compiler_configuration(target)
            cases = [(f'target_os="{name}"', os_name == name) for name in ('linux', 'macos', 'windows')]
            cases += [('any(unix, windows)', True), ('any(test, target_os="linux")', os_name == 'linux'),
                      ('not(any(target_os="linux", target_os="macos", target_os="windows"))', False),
                      ('not(any(unix, windows))', False), ('all(test, unix)', False),
                      ('not(test)', True), ('not(unix)', os_name == 'windows')]
            for predicate, active in cases:
                source = '#[cfg(' + predicate + ')] fn f() {}\n'
                inventory = self.inventory(source, target)
                self.assertEqual([row['name'] for row in inventory['symbols']], ['f'] if active else [])
                self.assertEqual([self.cfg_source_slice(source, span) for span in inventory['excluded']],
                                 [] if active else [source.rstrip()])
            refusals = {}
            for index, predicate in enumerate(rejected):
                file = evidence / (target + '-' + str(index) + '.rs')
                file.write_text('#[cfg(' + predicate + ')] fn f() {}\n')
                result = subprocess.run([str(self.binary), str(file), '--target-cfg'],
                    input=json.dumps(configuration).encode(), capture_output=True, timeout=30)
                file.with_suffix('.stdout').write_bytes(result.stdout)
                file.with_suffix('.stderr').write_bytes(result.stderr)
                self.assertNotEqual(result.returncode, 0, (target, predicate))
                self.assertEqual(result.stdout, b'')
                self.assertIn(b'unsupported cfg', result.stderr)
                refusals[predicate] = {'returncode': result.returncode}
            results[target] = {'configuration': configuration, 'refusals': refusals}
        (evidence / 'results.json').write_text(json.dumps(results, indent=2) + '\n')

    def test_cfg_whole_match_arms_keep_cc_owners_ranges_and_same_line_neighbor(self):
        from function_risk import own_lines
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh286-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='whole-match-ast-', dir=retained))
        results = {}
        arms = [line.strip() for line in CFG_MATCH_ARM_SOURCE.splitlines()[2:6]]
        arms[-1] = arms[-1].split(' 1 =>')[0]
        for target, active in (('x86_64-unknown-linux-gnu', 0),
                               ('aarch64-apple-darwin', 1), ('x86_64-pc-windows-msvc', 2)):
            inventory = self.inventory(CFG_MATCH_ARM_SOURCE, target)
            symbols = inventory['symbols']
            self.assertEqual([row['kind'] for row in symbols], ['function', 'closure', 'closure', 'function'])
            self.assertEqual([row['raw'] for row in symbols[:1]],
                             [{'match': 1, 'match_arms': 3, 'match_decisions': 2, 'guards': 1, 'closures': 2}])
            self.assertEqual([complexity(row['raw']) for row in symbols], [4, 2, 1, 1])
            self.assertEqual([row['name'] for row in symbols if row['kind'] == 'function'], ['configured', 'unused'])
            self.assertIn('hot', symbols[0]['syntax'])
            self.assertNotIn('hidden', symbols[0]['syntax'])
            self.assertNotIn('# [cfg', symbols[0]['syntax'])
            self.assertEqual(symbols[1]['span'][0], active + 3)
            self.assertEqual(symbols[2]['span'][0], 6)
            actual = [self.cfg_source_slice(CFG_MATCH_ARM_SOURCE, span) for span in inventory['excluded']]
            self.assertEqual(actual, [arm for index, arm in enumerate(arms) if index != active])
            transformed, edits = instrument(CFG_MATCH_ARM_SOURCE, inventory)
            reparsed = self.inventory(transformed, target)
            self.assertEqual([row['raw'] for row in reparsed['symbols']], [row['raw'] for row in symbols])
            remapped = [(*original_point(byte_span(transformed, span)[:2], edits),
                         *original_point(byte_span(transformed, span)[2:], edits)) for span in reparsed['excluded']]
            spans = [byte_span(CFG_MATCH_ARM_SOURCE, span) for span in inventory['excluded']]
            self.assertEqual(remapped, spans)
            # Synthetic byte intervals are a separate oracle, never inserted
            # into a native LLVM export. Inactive bytes carry no denominator;
            # the immediately adjacent cold closure on line 6 still counts.
            for span in spans:
                self.assertEqual(own_lines({(*span, 0): 1}, spans), {})
            cold = byte_span(CFG_MATCH_ARM_SOURCE, symbols[2]['span'])
            self.assertEqual(own_lines({(*cold, 0): 1}, spans), {6: 1})
            results[target] = {'inventory': inventory, 'excluded_bytes': actual,
                               'synthetic_only': {'cold_lines': [6], 'inactive_lines': []}}
        (evidence / 'source.rs').write_text(CFG_MATCH_ARM_SOURCE)
        (evidence / 'results.json').write_text(json.dumps(results, indent=2) + '\n')

    def test_cfg_match_arm_unknown_and_residual_positions_fail_without_partial_output(self):
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh286-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='match-arm-refusals-', dir=retained))
        for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'):
            for index, arm in enumerate((
                '#[cfg(windows)] #[cfg(feature="extra")] 0 => 1,',
                '#[cfg(any(unix, feature="extra"))] 0 => 1,',
                '#[cfg_attr(unix, inline)] 0 => 1,',
                '0 if #[cfg(unix)] true => 1,',
                '0 => (#[cfg(unix)] 1),',
                '0 if { let _ = #[cfg(unix)] checked(true)?; true } => 1,',
            )):
                file = evidence / (target + '-' + str(index) + '.rs')
                file.write_text('fn checked(ok: bool) -> Result<(), ()> { Ok(()) }\n'
                                + 'fn f() { match 0 { ' + arm + ' _ => 2, }; }\n')
                result = subprocess.run([str(self.binary), str(file), '--target-cfg'],
                    input=json.dumps(compiler_configuration(target)).encode(), capture_output=True, timeout=30)
                file.with_suffix('.stdout').write_bytes(result.stdout)
                file.with_suffix('.stderr').write_bytes(result.stderr)
                self.assertNotEqual(result.returncode, 0, (target, arm))
                self.assertEqual(result.stdout, b'')
                self.assertIn(b'unsupported', result.stderr)

    def test_cfg_whole_match_arm_real_native_independent_and_zero_counters(self):
        from function_risk import contains
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh286-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='whole-match-native-', dir=retained))
        crate = evidence / 'crate'
        (crate / 'src').mkdir(parents=True)
        file = crate / 'src/configured.rs'
        file.write_bytes(CFG_MATCH_ARM_SOURCE.encode())
        configuration = compiler_configuration()
        inventory = ast(file, self.binary, configuration)
        self.assertEqual([row['kind'] for row in inventory['symbols']],
                         ['function', 'closure', 'closure', 'function'])
        transformed, edits = instrument(CFG_MATCH_ARM_SOURCE, inventory)
        file.write_bytes(transformed.encode())
        manifest = {'series': SERIES, 'configuration': configuration, 'files': {'configured.rs': {
            'original': CFG_MATCH_ARM_SOURCE, 'original_sha256': digest(CFG_MATCH_ARM_SOURCE.encode()),
            'instrumented_sha256': digest(transformed.encode()), 'inventory': inventory, 'edits': edits}}}
        (evidence / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        (evidence / 'original.rs').write_bytes(CFG_MATCH_ARM_SOURCE.encode())
        main = crate / 'src/main.rs'
        main.write_text('''mod configured;
fn main() {
    let mode = std::env::args().nth(1).unwrap();
    if mode == "unused" { assert_eq!(configured::unused(), 11); }
    else {
        let (value, expected) = match mode.as_str() { "hot" => (0, 7), "cold" => (1, 9), "zero" => (2, 3), _ => panic!() };
        assert_eq!(configured::configured(value), expected);
    }
}
''')
        commands = []

        def run(argv, **kwargs):
            record = {'argv': [str(arg) for arg in argv], 'cwd': str(crate),
                      'environment': kwargs.pop('record_environment', {})}
            commands.append(record)
            try:
                result = subprocess.run(argv, cwd=crate, capture_output=True, timeout=120, **kwargs)
            except (OSError, subprocess.TimeoutExpired) as error:
                record['error'] = str(error)
                if isinstance(error, subprocess.TimeoutExpired):
                    (evidence / f'command-{len(commands)}.stdout').write_bytes(error.stdout or b'')
                    (evidence / f'command-{len(commands)}.stderr').write_bytes(error.stderr or b'')
                (evidence / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
                raise
            record['returncode'] = result.returncode
            (evidence / f'command-{len(commands)}.stdout').write_bytes(result.stdout)
            (evidence / f'command-{len(commands)}.stderr').write_bytes(result.stderr)
            (evidence / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
            self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
            return result.stdout

        suffix = '.exe' if os.name == 'nt' else ''
        executable = crate / ('whole-match-native' + suffix)
        run(['rustc', '--edition=2021', '--target', configuration['target'], '-C', 'instrument-coverage',
             '-C', 'opt-level=0', str(main), '-o', str(executable)], env=os.environ.copy())
        sysroot = Path(run(['rustc', '--print', 'sysroot']).decode().strip())
        version = run(['rustc', '-vV']).decode().strip()
        host = next(line.removeprefix('host: ') for line in version.splitlines() if line.startswith('host: '))
        tools = {name: sysroot / 'lib/rustlib' / host / 'bin' / (name + suffix) for name in ('llvm-cov', 'llvm-profdata')}
        versions = {name: run([str(path), '--version']).decode().strip() for name, path in tools.items()}
        exclusions = [byte_span(CFG_MATCH_ARM_SOURCE, span) for span in inventory['excluded']]
        results, negatives, real_denominators = {}, [], {}
        for mode, expected in (('hot', [1, 1, 0, 0]), ('cold', [1, 0, 1, 0]),
                               ('zero', [1, 0, 0, 0]), ('unused', [0, 0, 0, 1])):
            raw, profile = evidence / f'{mode}.profraw', evidence / f'{mode}.profdata'
            run([str(executable), mode], env={**os.environ, 'LLVM_PROFILE_FILE': str(raw)},
                record_environment={'LLVM_PROFILE_FILE': str(raw)})
            run([str(tools['llvm-profdata']), 'merge', '-sparse', str(raw), '-o', str(profile)])
            exported = run([str(tools['llvm-cov']), 'export', str(executable), f'-instr-profile={profile}'])
            (evidence / f'{mode}.llvm.json').write_bytes(exported)
            llvm = json.loads(exported)
            rows = measure(manifest, llvm, crate, self.binary)
            self.assertEqual(len(rows), 4)
            self.assertEqual([row['cc'] for row in rows], [4, 2, 1, 1])
            self.assertEqual([len(row['instances']) for row in rows], [1, 1, 1, 1])
            self.assertEqual([row['instances'][0]['count'] for row in rows], expected)
            self.assertEqual(len({row['instances'][0]['index'] for row in rows}), 4)
            denominators = {}
            for row, count in zip(rows, expected):
                self.assertGreater(row['lines']['count'], 0)
                self.assertGreater(row['regions']['count'], 0)
                if count == 0:
                    self.assertEqual(row['lines']['covered'], 0)
                    self.assertEqual(row['regions']['covered'], 0)
                # Independent byte enumeration of REAL exported regions. This
                # is separate from the synthetic interval test above.
                span = byte_span(CFG_MATCH_ARM_SOURCE, row['span'])
                children = exclusions + [byte_span(CFG_MATCH_ARM_SOURCE, symbol['span'])
                    for symbol in inventory['symbols'] if symbol['span'] != row['span']
                    and contains(span, byte_span(CFG_MATCH_ARM_SOURCE, symbol['span']))]
                regions = {(*region['span'], region['kind']): region['count'] for region in row['raw_regions']}
                lines = {}
                for line_number, line in enumerate(CFG_MATCH_ARM_SOURCE.encode().splitlines(keepends=True), 1):
                    for column in range(1, len(line) + 1):
                        point = (line_number, column)
                        if any(child[:2] <= point < child[2:] for child in children):
                            continue
                        candidates = [(region, hits) for region, hits in regions.items()
                                      if region[:2] <= point < region[2:4]]
                        if not candidates:
                            continue
                        inner = [(region, hits) for region, hits in candidates
                                 if all(other[:2] <= region[:2] and region[2:4] <= other[2:4]
                                        for other, _ in candidates)]
                        self.assertEqual(len(inner), 1, 'real fixture requires unambiguous nested byte regions')
                        region, hits = inner[0]
                        if region[4] == 0:
                            lines[line_number] = max(lines.get(line_number, 0), hits)
                self.assertEqual(row['lines']['count'], len(lines))
                self.assertEqual(row['lines']['covered'], sum(bool(hits) for hits in lines.values()))
                if row['kind'] == 'closure' and row['span'][0] == 6:
                    self.assertEqual(set(lines), {6}, 'active neighbor retains its real line denominator')
                denominators[row['name']] = lines
                missing = copy.deepcopy(llvm)
                missing['data'][0]['functions'].pop(row['instances'][0]['index'])
                with self.assertRaisesRegex(ValueError, re.escape('missing LLVM function: configured.rs:' + row['name'] + ':')):
                    measure(manifest, missing, crate, self.binary)
                negatives.append({'mode': mode, 'missing_owner': row['name'], 'counter': count})
            parent = rows[0]
            for child in rows[1:3]:
                borrowed = copy.deepcopy(llvm)
                function = borrowed['data'][0]['functions'][child['instances'][0]['index']]
                native_parent = llvm['data'][0]['functions'][parent['instances'][0]['index']]
                function['regions'] = copy.deepcopy(native_parent['regions'])
                function['filenames'] = copy.deepcopy(native_parent['filenames'])
                with self.assertRaisesRegex(ValueError, 'function kind mismatch: configured.rs:configured'):
                    measure(manifest, borrowed, crate, self.binary)
            results[mode], real_denominators[mode] = rows, denominators
            (evidence / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
            (evidence / 'negative-checks.json').write_text(json.dumps(negatives, indent=2) + '\n')
            (evidence / 'real-denominators.json').write_text(json.dumps(real_denominators, indent=2) + '\n')
        tampered = copy.deepcopy(manifest)
        tampered['files']['configured.rs']['inventory']['excluded'] = []
        with self.assertRaisesRegex(ValueError, 'AST inventory does not reproduce'):
            measure(tampered, llvm, crate, self.binary)
        (evidence / 'validated.json').write_text(json.dumps({'series': SERIES,
            'analyzer_build_record': {'path': str(self.build_record), 'sha256': digest(self.build_record.read_bytes())},
            'compiler_configuration': configuration, 'binary_sha256': digest(executable.read_bytes()),
            'source_sha256': digest(CFG_MATCH_ARM_SOURCE.encode()), 'test_source_sha256': digest(Path(__file__).read_bytes()),
            'llvm_tools': {name: {'path': str(path), 'sha256': digest(path.read_bytes()), 'version': versions[name]}
                           for name, path in tools.items()},
            'boundaries': ['Host-native only; three-target AST is not three-platform native validation.',
                           'Real LLVM exports remain separate from synthetic interval oracles.']}, indent=2) + '\n')

    def test_cfg_try_statements_and_nested_positions_are_bounded(self):
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh285-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='cfg-try-ast-', dir=retained))
        results = {}
        for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'):
            configuration = compiler_configuration(target)
            inventory = self.inventory(CFG_TRY_SOURCE, target)
            (evidence / f'{target}.inventory.json').write_text(json.dumps(inventory, indent=2) + '\n')
            symbols = inventory['symbols']
            self.assertEqual([s['name'] for s in symbols], ['checked', 'configured'])
            self.assertEqual([complexity(s['raw']) for s in symbols], [2, 2])
            self.assertEqual(symbols[1]['raw']['question_mark'], 1)
            self.assertEqual(len(inventory['excluded']), 1)
            span = inventory['excluded'][0]
            lines = CFG_TRY_SOURCE.splitlines(keepends=True)
            start = sum(len(line) for line in lines[:span[0] - 1]) + span[1] - 1
            end = sum(len(line) for line in lines[:span[2] - 1]) + span[3] - 1
            excluded = CFG_TRY_SOURCE[start:end]
            inactive = 'unix' if 'windows' in configuration['cfg'] else 'windows'
            self.assertEqual(excluded, f'#[cfg({inactive})]\n    checked(\n        ok\n    )?;',
                             'excluded range must contain the complete statement, including ? and semicolon')
            transformed, edits = instrument(CFG_TRY_SOURCE, inventory)
            self.assertEqual(transformed, CFG_TRY_SOURCE)
            self.assertEqual(edits, [])
            self.assertEqual(self.inventory(transformed, target), inventory)
            results[target] = {'excluded_statement': excluded, 'byte_span': byte_span(CFG_TRY_SOURCE, span)}
            cases = [
                ('nested-true', f'let _value = #[cfg({"windows" if inactive == "unix" else "unix"})] checked(ok)?;',
                 'unsupported cfg try expression position; only removable statements are supported'),
                ('nested-false', f'let _value = #[cfg({inactive})] checked(ok)?;',
                 'unsupported cfg try expression position; only removable statements are supported'),
                ('nested-cfg-attr', 'let _value = #[cfg_attr(unix, inline)] checked(ok)?;',
                 'unsupported cfg try expression position; only removable statements are supported'),
                ('statement-unknown', '#[cfg(feature="extra")] checked(ok)?;', 'unsupported cfg predicate'),
                ('statement-cfg-attr', '#[cfg_attr(unix, inline)] checked(ok)?;', 'unsupported cfg_attr'),
            ]
            for name, statement, error in cases:
                directory = evidence / target / name
                directory.mkdir(parents=True)
                file = directory / 'input.rs'
                file.write_text('fn checked(ok: bool) -> Result<(), ()> { if ok { Ok(()) } else { Err(()) } }\n'
                                + 'fn run(ok: bool) -> Result<(), ()> { ' + statement + ' Ok(()) }\n')
                result = subprocess.run([str(self.binary), str(file), '--target-cfg'],
                                        input=json.dumps(configuration), capture_output=True, text=True, timeout=30)
                (directory / 'stdout.log').write_text(result.stdout)
                (directory / 'stderr.log').write_text(result.stderr)
                (directory / 'status.json').write_text(json.dumps({'returncode': result.returncode,
                    'expected_error': error, 'configuration': configuration, 'binary_sha256': digest(self.binary.read_bytes())}) + '\n')
                self.assertNotEqual(result.returncode, 0, (target, name))
                self.assertIn(error, result.stderr, (target, name))
                self.assertEqual(result.stdout, '', 'rejection may not emit a partial production inventory')
        (evidence / 'source.rs').write_bytes(CFG_TRY_SOURCE.encode("utf-8"))
        (evidence / 'validated.json').write_text(json.dumps(results, indent=2) + '\n')

    def test_cfg_try_synthetic_interval_contract(self):
        from function_risk import own_lines
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh285-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='cfg-try-synthetic-', dir=retained))
        (evidence / 'source.rs').write_bytes(CFG_TRY_SOURCE.encode("utf-8"))
        before_lines = [4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        # This is deliberately synthetic: LLVM's actual export has separate
        # regions, not this enclosing interval. Never add it to a native export.
        region = (4, 1, 15, 2, 0)
        cases = {}
        for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc'):
            configuration = compiler_configuration(target)
            inventory = self.inventory(CFG_TRY_SOURCE, target)
            (evidence / f'{target}.inventory.json').write_text(json.dumps(inventory, indent=2) + '\n')
            self.assertEqual(len(inventory['excluded']), 1)
            excluded = byte_span(CFG_TRY_SOURCE, inventory['excluded'][0])
            if 'windows' in configuration['cfg']:
                expected_excluded = (6, 5, 9, 8)
                after_lines = [4, 5, 6, 9, 10, 11, 12, 13, 14, 15]
            else:
                expected_excluded = (10, 5, 13, 8)
                after_lines = [4, 5, 6, 7, 8, 9, 10, 13, 14, 15]
            self.assertEqual(excluded, expected_excluded)
            before = own_lines({region: 1}, [])
            after = own_lines({region: 1}, [excluded])
            cases[target] = {'configuration': configuration, 'actual_ast_excluded': list(excluded),
                'synthetic_region': list(region), 'synthetic_count': 1,
                'expected_before_lines': before_lines, 'expected_after_lines': after_lines,
                'observed_before': before, 'observed_after': after}
            report = {'evidence_kind': 'synthetic-interval-unit-contract',
                'not_native_llvm_evidence': True, 'not_repository_risk_evidence': True,
                'source_sha256': digest(CFG_TRY_SOURCE.encode()),
                'test_source_sha256': digest(Path(__file__).read_bytes()), 'cases': cases}
            (evidence / 'synthetic-contract.json').write_text(json.dumps(report, indent=2) + '\n')
            self.assertEqual(before, {line: 1 for line in before_lines})
            self.assertEqual(after, {line: 1 for line in after_lines})
            self.assertEqual((len(before), len(after)), (12, 10))
            self.assertTrue({excluded[0], excluded[2]}.issubset(after),
                            'shared boundary lines retain the unexcluded parent intervals')
            self.assertTrue(set(range(excluded[0] + 1, excluded[2])).isdisjoint(after),
                            'whole interior lines have no synthetic interval contribution')
        (evidence / 'validated.json').write_text(json.dumps({'evidence_kind': 'synthetic-interval-unit-contract',
            'validated_targets': list(cases)}, indent=2) + '\n')

    def test_cfg_try_real_native_ok_err_and_excluded_denominators(self):
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh285-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='cfg-try-native-', dir=retained))
        crate = evidence / 'crate'
        (crate / 'src').mkdir(parents=True)
        file = crate / 'src/configured.rs'
        file.write_bytes(CFG_TRY_SOURCE.encode("utf-8"))
        self.assertEqual(file.read_bytes(), CFG_TRY_SOURCE.encode("utf-8"))
        configuration = compiler_configuration()
        inventory = ast(file, self.binary, configuration)
        transformed, edits = instrument(CFG_TRY_SOURCE, inventory)
        file.write_bytes(transformed.encode("utf-8"))
        self.assertEqual(file.read_bytes(), transformed.encode("utf-8"))
        self.assertEqual(edits, [])
        self.assertEqual(len(inventory['excluded']), 1)
        manifest = {'series': SERIES, 'configuration': configuration, 'files': {'configured.rs': {
            'original': CFG_TRY_SOURCE, 'original_sha256': digest(CFG_TRY_SOURCE.encode()),
            'instrumented_sha256': digest(transformed.encode()), 'inventory': inventory, 'edits': edits}}}
        (evidence / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        (evidence / 'configured.original.rs').write_bytes(CFG_TRY_SOURCE.encode("utf-8"))
        main = crate / 'src/main.rs'
        main.write_text('''mod configured;
fn main() {
    let ok = std::env::args().nth(1).unwrap() == "ok";
    assert_eq!(configured::configured(ok), if ok { Ok(7) } else { Err(()) });
}
''')
        commands = []

        def run(argv, **kwargs):
            record = {'argv': [str(arg) for arg in argv], 'cwd': str(crate),
                      'environment': kwargs.pop('record_environment', {})}
            commands.append(record)
            try:
                result = subprocess.run(argv, cwd=crate, capture_output=True, timeout=120, **kwargs)
            except (OSError, subprocess.TimeoutExpired) as error:
                record['error'] = str(error)
                if isinstance(error, subprocess.TimeoutExpired):
                    (evidence / f'command-{len(commands)}.stdout').write_bytes(error.stdout or b'')
                    (evidence / f'command-{len(commands)}.stderr').write_bytes(error.stderr or b'')
                (evidence / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
                raise
            record['returncode'] = result.returncode
            (evidence / f'command-{len(commands)}.stdout').write_bytes(result.stdout)
            (evidence / f'command-{len(commands)}.stderr').write_bytes(result.stderr)
            (evidence / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
            self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
            return result.stdout

        suffix = '.exe' if os.name == 'nt' else ''
        executable = crate / ('cfg-try-native' + suffix)
        run(['rustc', '--edition=2021', '--target', configuration['target'], '-C', 'instrument-coverage',
             '-C', 'opt-level=0', str(main), '-o', str(executable)], env=os.environ.copy())
        sysroot = Path(run(['rustc', '--print', 'sysroot']).decode().strip())
        version = run(['rustc', '-vV']).decode().strip()
        host = next(line.removeprefix('host: ') for line in version.splitlines() if line.startswith('host: '))
        tools = {name: sysroot / 'lib/rustlib' / host / 'bin' / (name + suffix) for name in ('llvm-cov', 'llvm-profdata')}
        versions = {name: run([str(path), '--version']).decode().strip() for name, path in tools.items()}
        results, exports, negatives, denominator_oracles = {}, {}, [], {}
        excluded = byte_span(CFG_TRY_SOURCE, inventory['excluded'][0])
        interior_lines = set(range(excluded[0] + 1, excluded[2]))

        def expected_lines(regions):
            # Independent oracle: enumerate original byte positions, selecting
            # the smallest containing LLVM interval at each point. Production
            # own_lines instead sweeps ordered interval endpoints. This fixture
            # has nested intervals; crossing/equal ownership is an error.
            source_lines = CFG_TRY_SOURCE.encode().splitlines(keepends=True)
            expected = {}
            for line_number, line in enumerate(source_lines, 1):
                for column in range(1, len(line) + 1):
                    point = (line_number, column)
                    if excluded[:2] <= point < excluded[2:]:
                        continue
                    candidates = [(span, hits) for span, hits in regions.items() if span[:2] <= point < span[2:4]]
                    if not candidates:
                        continue
                    innermost = [(span, hits) for span, hits in candidates
                                 if all(other[:2] <= span[:2] and span[2:4] <= other[2:4]
                                        for other, _ in candidates)]
                    self.assertEqual(len(innermost), 1, 'fixture oracle requires independent unambiguous nested regions')
                    span, hits = innermost[0]
                    if span[4] == 0:
                        expected[line_number] = max(expected.get(line_number, 0), hits)
            return expected
        for mode in ('ok', 'err', 'combined'):
            if mode != 'combined':
                raw = evidence / f'{mode}.profraw'
                run([str(executable), mode], env={**os.environ, 'LLVM_PROFILE_FILE': str(raw)},
                    record_environment={'LLVM_PROFILE_FILE': str(raw)})
                files = [raw]
            else:
                files = [evidence / 'ok.profraw', evidence / 'err.profraw']
            profile = evidence / f'{mode}.profdata'
            run([str(tools['llvm-profdata']), 'merge', '-sparse', *map(str, files), '-o', str(profile)])
            exported = run([str(tools['llvm-cov']), 'export', str(executable), f'-instr-profile={profile}'])
            (evidence / f'{mode}.llvm.json').write_bytes(exported)
            native = json.loads(exported)
            rows = measure(manifest, native, crate, self.binary)
            self.assertEqual([row['name'] for row in rows], ['checked', 'configured'])
            self.assertEqual([row['cc'] for row in rows], [2, 2])
            for row in rows:
                self.assertEqual(len(row['instances']), 1)
                self.assertEqual(row['instances'][0]['count'], 2 if mode == 'combined' else 1)
                self.assertGreater(row['lines']['count'], 0)
                self.assertGreater(row['regions']['count'], 0)
            selected = rows[1]
            fn = native['data'][0]['functions'][selected['instances'][0]['index']]
            regions = {}
            for region in fn['regions']:
                if region[:2] != region[2:4]:
                    span = (*region[:4], region[7])
                    regions[span] = max(regions.get(span, 0), region[4])
            owned = {span: count for span, count in regions.items()
                     if not (excluded[:2] <= span[:2] and span[2:4] <= excluded[2:])}
            lines = expected_lines(owned)
            overlapping = [{'span': list(span[:4]), 'kind': span[4], 'count': count}
                           for span, count in sorted(regions.items())
                           if span[:2] < excluded[2:] and excluded[:2] < span[2:4]]
            denominator_oracles[mode] = {'excluded_interval': list(excluded),
                'whole_interior_lines': sorted(interior_lines), 'line_counts': lines,
                'actual_llvm_regions': fn['regions'], 'overlapping_regions': overlapping,
                'native_denominator_reduction_certified': False,
                'code_regions': [{'span': list(span[:4]), 'count': count}
                                 for span, count in sorted(owned.items()) if span[4] == 0]}
            (evidence / 'denominator-oracles.json').write_text(json.dumps(denominator_oracles, indent=2) + '\n')
            self.assertEqual(overlapping, [], 'actual LLVM regions must not overlap the inactive full statement')
            self.assertTrue(interior_lines.isdisjoint(lines), 'inactive interior lines must be absent from actual native denominator')
            expected_line_set = {4, 5, 11, 12, 13, 14, 15} if 'windows' in configuration['cfg'] else {4, 5, 7, 8, 9, 14, 15}
            self.assertEqual(set(lines), expected_line_set)
            self.assertEqual(selected['lines']['count'], len(lines))
            self.assertEqual(selected['lines']['covered'], sum(count > 0 for count in lines.values()))
            self.assertEqual(selected['regions']['count'], sum(span[4] == 0 for span in owned))
            self.assertEqual(selected['regions']['covered'], sum(span[4] == 0 and count > 0 for span, count in owned.items()))
            self.assertEqual([region for region in selected['raw_regions'] if region['kind'] == 0],
                             [{'span': list(span[:4]), 'kind': 0, 'count': count}
                              for span, count in sorted(owned.items()) if span[4] == 0])
            results[mode], exports[mode] = rows, native
            (evidence / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
        # A physical line can remain covered by enclosing intervals even when
        # its return expression is not reached. Certify the actual return-region
        # counter, rather than demand a decrease in the line-union coverage.
        return_line = next(index for index, line in enumerate(CFG_TRY_SOURCE.splitlines(), 1)
                           if line.strip() == 'Ok(7)')
        return_point = (return_line, CFG_TRY_SOURCE.splitlines()[return_line - 1].index('7') + 1)
        ok_regions = {tuple(region['span']): region['count'] for region in results['ok'][1]['raw_regions']
                      if region['kind'] == 0}
        return_witnesses = [{'span': region['span'], 'err_count': region['count'],
                             'ok_count': ok_regions[tuple(region['span'])]}
                            for region in results['err'][1]['raw_regions']
                            if region['kind'] == 0 and region['count'] == 0
                            and tuple(region['span'][:2]) <= return_point < tuple(region['span'][2:])
                            and ok_regions.get(tuple(region['span']), 0) > 0]
        self.assertTrue(return_witnesses,
                        'Err must have an independent zero-count Ok(7) region that Ok executes')
        for row in results['combined']:
            missing = copy.deepcopy(exports['combined'])
            missing['data'][0]['functions'].pop(row['instances'][0]['index'])
            with self.assertRaisesRegex(ValueError, re.escape('missing LLVM function: configured.rs:' + row['name'] + ':')):
                measure(manifest, missing, crate, self.binary)
            negatives.append('missing independent owner: ' + row['name'])
        tampered = copy.deepcopy(manifest)
        tampered['files']['configured.rs']['inventory']['excluded'] = []
        with self.assertRaisesRegex(ValueError, 'AST inventory does not reproduce'):
            measure(tampered, exports['combined'], crate, self.binary)
        negatives.append('omitted statement exclusion rejected at AST reproduction')
        for identity in ('compiler-target-production/1', 'compiler-target-production/2', 'compiler-target-production/3'):
            old = copy.deepcopy(manifest)
            old['series']['configuration'] = identity
            with self.assertRaisesRegex(ValueError, 'incompatible measurement series'):
                measure(old, exports['combined'], crate, self.binary)
            negatives.append(identity + ' rejected before mapping')
        report = {'series': SERIES, 'configuration': configuration, 'tools': provenance(self.binary),
                  'analyzer_build_record': {'path': str(self.build_record), 'sha256': digest(self.build_record.read_bytes())},
                  'binary_sha256': digest(executable.read_bytes()), 'test_source_sha256': digest(Path(__file__).read_bytes()),
                  'llvm_tools': {name: {'path': str(path.resolve()), 'version': versions[name], 'sha256': digest(path.read_bytes())}
                                 for name, path in tools.items()}, 'results': results,
                  'independent_denominator_oracles': denominator_oracles,
                  'early_return_regions': return_witnesses, 'validated': negatives}
        (evidence / 'native-result.json').write_text(json.dumps(report, indent=2) + '\n')

    def test_process_command_real_fork_exec_native_ownership(self):
        # Only this fixture explicitly writes child counters before exec. The
        # production callback and ordinary risk profiles are never changed.
        driver = r'''mod command;
use std::process::Command;
#[cfg(unix)]
unsafe extern "C" {
    fn __llvm_profile_set_filename(name: *const std::ffi::c_char);
    fn __llvm_profile_write_file() -> std::ffi::c_int;
}
fn main() {
    let mode = std::env::args().nth(1).unwrap();
    #[cfg(unix)]
    let mut cmd = {
        if mode == "live-anchor" {
            let mut cmd = Command::new("python3");
            // Publish readiness only after the complete PID has been written.
            // Creating the final file first lets the parent observe empty bytes.
            cmd.args(["-c", "import os,signal,time; from pathlib import Path; signal.signal(signal.SIGTERM,lambda *_:os._exit(7)); ready=Path(os.environ['LIVE_READY']); pending=ready.with_suffix('.tmp'); pending.write_text(str(os.getpid())); pending.replace(ready); time.sleep(30)"]);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", if mode == "kill-error" { "exec sleep 30" } else { "exit 7" }]);
            cmd
        }
    };
    #[cfg(windows)]
    let mut cmd = {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "ping -n 30 127.0.0.1 > nul"]);
        cmd
    };
    if mode != "plain" {
        command::isolate_process_tree(&mut cmd);
        #[cfg(unix)] {
            use std::os::unix::process::CommandExt;
            let filename = std::ffi::CString::new(std::env::var("CHILD_PROFILE").unwrap()).unwrap();
            // Registration order: original setsid closure, then this callback.
            // This single-threaded fixture deliberately calls the LLVM profiler
            // runtime after fork. It is not an async-signal-safe production hook.
            unsafe {
                cmd.pre_exec(move || {
                    __llvm_profile_set_filename(filename.as_ptr());
                    if __llvm_profile_write_file() != 0 {
                        return Err(std::io::Error::other("child profile write failed"));
                    }
                    Ok(())
                });
            }
        }
    }
    let mut child = cmd.spawn().unwrap();
    #[cfg(unix)] if mode == "live-anchor" {
        // A real owned live member, with the TERM handler installed before
        // readiness, certifies both unmodified base and head callbacks.
        let path = std::path::PathBuf::from(std::env::var("LIVE_READY").unwrap());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Ok(pid) = std::fs::read_to_string(&path) {
                assert_eq!(pid.parse::<u32>().unwrap(), child.id());
                break;
            }
            assert!(std::time::Instant::now() < deadline, "live-anchor readiness timeout");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    #[cfg(unix)] if mode != "kill-error" && mode != "live-anchor" {
        // Observe natural exit without reaping, including for the exact base
        // implementation. TERM cannot race the intended exit-7 assertion.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            assert_eq!(unsafe { libc::waitid(libc::P_PID, child.id() as libc::id_t, &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT) }, 0);
            if unsafe { info.si_pid() } != 0 { break; }
            assert!(std::time::Instant::now() < deadline, "natural-exit readiness timeout");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    HEAD_CASES
    let outcome = command::terminate(&mut child);
    #[cfg(target_os = "macos")]
    if BASE_CASE && mode == "isolated" {
        // Preserve the exact old implementation. This host's real raw outcome
        // is evidence, not a claim that the base already fixed zombie groups.
        match outcome {
            Ok(status) => {
                assert_eq!(status.code(), Some(7));
                println!("outcome=ok status=7");
            }
            Err(error) => {
                assert_eq!(error.raw_os_error(), Some(libc::EPERM));
                println!("outcome=eperm status=7");
                // No group signals after reap; WNOWAIT above proved exited.
                assert_eq!(child.wait().unwrap().code(), Some(7));
            }
        }
        assert!(child.try_wait().unwrap().is_some());
        return;
    }
    let status = outcome.unwrap();
    #[cfg(unix)] {
        assert_eq!(status.code(), Some(7));
        println!("outcome=ok status=7");
    }
    #[cfg(windows)] {
        assert!(!status.success());
        println!("outcome=terminated");
    }
    assert!(child.try_wait().unwrap().is_some());
}
'''
        head_cases = r'''
    #[cfg(target_os = "macos")]
    if mode == "guard-checks" {
        fn exited(_: i32) -> std::io::Result<bool> { Ok(true) }
        fn live(_: i32) -> std::io::Result<bool> { Ok(false) }
        fn reaped(_: i32) -> std::io::Result<bool> { Err(std::io::Error::from_raw_os_error(libc::ECHILD)) }
        fn denied(_: i32) -> std::io::Result<bool> { Err(std::io::Error::from_raw_os_error(libc::EPERM)) }
        fn forbidden(_: i32, _: &mut [libc::pid_t; 2]) -> i32 { panic!("query after unknown/live leader"); }
        fn empty(_: i32, _: &mut [libc::pid_t; 2]) -> i32 { 0 }
        fn negative(_: i32, _: &mut [libc::pid_t; 2]) -> i32 { -1 }
        fn short(pid: i32, pids: &mut [libc::pid_t; 2]) -> i32 { pids[0] = pid; 2 }
        fn nonintegral(pid: i32, pids: &mut [libc::pid_t; 2]) -> i32 { pids[0] = pid; 7 }
        fn full(pid: i32, pids: &mut [libc::pid_t; 2]) -> i32 { *pids = [pid, pid + 1]; 8 }
        fn wrong(pid: i32, pids: &mut [libc::pid_t; 2]) -> i32 { pids[0] = pid + 1; 4 }
        for query in [empty, negative, short, nonintegral, full, wrong] {
            assert!(!command::darwin_group_is_finished(child.id() as i32, exited, query));
        }
        for observe in [live, reaped, denied] {
            assert!(!command::darwin_group_is_finished(child.id() as i32, observe, forbidden));
        }
        println!("guard_queries=6 observer_refusals=3");
    }
    #[cfg(unix)] {
        if mode == "reaped" {
            child.wait().unwrap();
            fn forbidden(_: i32, _: i32) -> std::io::Result<()> { panic!("signal after ECHILD"); }
            assert_eq!(command::terminate_with_signal(&mut child, forbidden, std::time::Duration::ZERO).unwrap_err().raw_os_error(), Some(libc::ECHILD));
            return;
        }
        if mode == "kill-error" {
            fn denied(_: i32, sig: i32) -> std::io::Result<()> {
                Err(std::io::Error::from_raw_os_error(if sig == libc::SIGTERM { libc::EACCES } else { libc::EPERM }))
            }
            let start = std::time::Instant::now();
            assert_eq!(command::terminate_with_signal(&mut child, denied, std::time::Duration::ZERO).unwrap_err().raw_os_error(), Some(libc::EACCES));
            assert!(start.elapsed() < std::time::Duration::from_secs(4));
            assert!(child.try_wait().unwrap().is_none());
            child.kill().unwrap();
            child.wait().unwrap();
            return;
        }
    }
'''
        repo_lock = (ROOT / 'tools/harness-gate/Cargo.lock').read_bytes()
        blocks = repo_lock.decode().split('[[package]]\n')[1:]
        libc_blocks = [block for block in blocks if tomllib.loads('[[package]]\n' + block)['package'][0]['name'] == 'libc']
        self.assertEqual(len(libc_blocks), 1)
        package = tomllib.loads('[[package]]\n' + libc_blocks[0])['package'][0]
        self.assertNotIn('dependencies', package)
        cargo = ('[package]\nname="process-native-fixture"\nversion="0.0.0"\nedition="2021"\n'
                 '[workspace]\n[dependencies]\nlibc="=' + package['version'] + '"\n')
        lock = ('version = 4\n\n[[package]]\nname = "process-native-fixture"\nversion = "0.0.0"\n'
                'dependencies = ["libc"]\n\n[[package]]\n' + libc_blocks[0]).encode()
        retained = Path(os.environ.get('RUST_MEASURE_NATIVE_EVIDENCE', ROOT / 'target/gh285-native-evidence')).resolve()
        retained.mkdir(parents=True, exist_ok=True)
        evidence = Path(tempfile.mkdtemp(prefix='process-command-', dir=retained))
        commands = []

        def run(argv, **kwargs):
            record = {'argv': [str(a) for a in argv], 'cwd': str(kwargs.get('cwd', ROOT)),
                      'environment': kwargs.pop('record_environment', {})}
            if 'input' in kwargs:
                record['stdin_sha256'] = digest(kwargs['input'])
            commands.append(record)
            try:
                result = subprocess.run(argv, capture_output=True, timeout=120, **kwargs)
            except (OSError, subprocess.TimeoutExpired) as error:
                record['error'] = str(error)
                if isinstance(error, subprocess.TimeoutExpired):
                    (evidence / f'command-{len(commands)}.stdout').write_bytes(error.stdout or b'')
                    (evidence / f'command-{len(commands)}.stderr').write_bytes(error.stderr or b'')
                (evidence / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
                raise
            record['returncode'] = result.returncode
            (evidence / f'command-{len(commands)}.stdout').write_bytes(result.stdout)
            (evidence / f'command-{len(commands)}.stderr').write_bytes(result.stderr)
            (evidence / 'commands.json').write_text(json.dumps(commands, indent=2) + '\n')
            self.assertEqual(result.returncode, 0, f'{argv}: {result.stderr.decode(errors="replace")}')
            return result.stdout

        configuration = compiler_configuration()
        suffix = '.exe' if os.name == 'nt' else ''
        sysroot = Path(run(['rustc', '--print', 'sysroot']).decode().strip())
        version = run(['rustc', '-vV']).decode().strip()
        host = next(line.removeprefix('host: ') for line in version.splitlines() if line.startswith('host: '))
        llvm = {name: sysroot / 'lib/rustlib' / host / 'bin' / (name + suffix) for name in ('llvm-profdata', 'llvm-cov')}
        tool_versions = {name: run([str(path), '--version']).decode().strip() for name, path in llvm.items()}
        report = {'series': SERIES, 'configuration': configuration, 'tools': provenance(self.binary),
                  'analyzer_build_record': {'path': str(self.build_record), 'sha256': digest(self.build_record.read_bytes())},
                  'checkout_sha': run(['git', 'rev-parse', 'HEAD']).decode().strip(),
                  'base_sha': PROCESS_COMMAND_BASE_COMMIT, 'base_source_sha256': PROCESS_COMMAND_BASE_SHA256,
                  'repository_lock_sha256': digest(repo_lock), 'fixture_lock_sha256': digest(lock),
                  'test_source_sha256': digest(Path(__file__).read_bytes()),
                  'llvm_tools': {name: {'path': str(path.resolve()), 'sha256': digest(path.read_bytes()),
                                'version': tool_versions[name]} for name, path in llvm.items()}, 'sources': {}}
        for label, source in self.process_command_sources().items():
            crate = evidence / label
            (crate / 'src').mkdir(parents=True)
            file = crate / 'src/command.rs'
            file.write_bytes(source.encode('utf-8'))
            self.assertEqual(file.read_bytes(), source.encode('utf-8'))
            inventory = json.loads(run([str(self.binary.resolve()), str(file), '--target-cfg'], input=json.dumps(configuration).encode()))
            transformed, edits = instrument(source, inventory)
            file.write_bytes(transformed.encode('utf-8'))
            self.assertEqual(file.read_bytes(), transformed.encode('utf-8'))
            manifest = {'series': SERIES, 'configuration': configuration, 'files': {'command.rs': {
                'original': source, 'original_sha256': digest(source.encode()),
                'instrumented_sha256': digest(transformed.encode()), 'inventory': inventory, 'edits': edits}}}
            (crate / 'command.original.rs').write_bytes(source.encode())
            (crate / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
            main_source = driver.replace('HEAD_CASES', head_cases if label == 'head' else '').replace('BASE_CASE', str(label == 'base').lower())
            (crate / 'src/main.rs').write_bytes(main_source.encode('utf-8'))
            self.assertEqual((crate / 'src/main.rs').read_bytes(), main_source.encode('utf-8'))
            (crate / 'Cargo.toml').write_text(cargo)
            (crate / 'Cargo.lock').write_bytes(lock)
            environment = {**os.environ, 'CARGO_TARGET_DIR': str(crate / 'target'), 'RUSTFLAGS': '-C instrument-coverage -C opt-level=0'}
            for key in ('CARGO_ENCODED_RUSTFLAGS', 'LLVM_PROFILE_FILE', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER'):
                environment.pop(key, None)
            run(['cargo', 'fetch', '--locked', '--manifest-path', str(crate / 'Cargo.toml'), '--target', configuration['target']],
                cwd=crate, env=environment, record_environment={'RUSTUP_TOOLCHAIN': environment.get('RUSTUP_TOOLCHAIN')})
            self.assertEqual((crate / 'Cargo.lock').read_bytes(), lock)
            run(['cargo', 'build', '--locked', '--offline', '--manifest-path', str(crate / 'Cargo.toml'), '--target', configuration['target']],
                cwd=crate, env=environment, record_environment={key: environment.get(key) for key in ('RUSTUP_TOOLCHAIN', 'CARGO_TARGET_DIR', 'RUSTFLAGS')})
            self.assertEqual((crate / 'Cargo.lock').read_bytes(), lock)
            executable = crate / 'target' / configuration['target'] / 'debug' / ('process-native-fixture' + suffix)
            results, negatives = {}, []
            modes = ['plain', 'isolated']
            if 'unix' in configuration['cfg']:
                modes += ['live-anchor']
            if label == 'head' and 'unix' in configuration['cfg']:
                modes += ['reaped', 'kill-error']
                if 'target_os="macos"' in configuration['cfg']:
                    modes += ['guard-checks']
            for mode in modes:
                directory = crate / mode
                directory.mkdir()
                parent_pattern, child_pattern = directory / 'parent-%p.profraw', directory / 'child-%p.profraw'
                mode_env = {'LLVM_PROFILE_FILE': str(parent_pattern), 'CHILD_PROFILE': str(child_pattern),
                            'LIVE_READY': str(directory / 'live-ready')}
                outcome = run([str(executable), mode], cwd=crate, env={**environment, **mode_env},
                              record_environment=mode_env).decode('utf-8').splitlines()
                darwin = 'target_os="macos"' in configuration['cfg']
                if mode in ('reaped', 'kill-error'):
                    self.assertEqual(outcome, [])
                elif mode == 'guard-checks':
                    self.assertEqual(outcome, ['guard_queries=6 observer_refusals=3', 'outcome=ok status=7'])
                elif darwin and label == 'base' and mode == 'isolated':
                    self.assertIn(outcome, [['outcome=eperm status=7'], ['outcome=ok status=7']])
                elif 'windows' in configuration['cfg']:
                    # taskkill inherits stdout; retain every raw line in outcome.
                    self.assertEqual([line for line in outcome if line.startswith('outcome=')],
                                     ['outcome=terminated'])
                else:
                    self.assertEqual(outcome, ['outcome=ok status=7'])
                parent_files, child_files = list(directory.glob('parent-*.profraw')), list(directory.glob('child-*.profraw'))
                self.assertEqual(len(parent_files), 1)
                unix = 'unix' in configuration['cfg']
                self.assertEqual(len(child_files), int(unix and mode != 'plain'))
                self.assertTrue(set(parent_files).isdisjoint(child_files))
                if child_files:
                    self.assertNotEqual(parent_files[0].stem.removeprefix('parent-'), child_files[0].stem.removeprefix('child-'))

                def export(name, files):
                    profile = directory / f'{name}.profdata'
                    run([str(llvm['llvm-profdata']), 'merge', '-sparse', *map(str, files), '-o', str(profile)])
                    raw = run([str(llvm['llvm-cov']), 'export', str(executable), f'-instr-profile={profile}'])
                    (directory / f'{name}.llvm.json').write_bytes(raw)
                    return json.loads(raw)

                parent_llvm = export('parent', parent_files)
                parent_rows = measure(manifest, parent_llvm, crate, self.binary)
                if unix:
                    parent_closure = next(row for row in parent_rows if row['kind'] == 'closure')
                    self.assertEqual(sum(i['count'] for i in parent_closure['instances']), 0,
                                     'parent snapshot does not observe post-fork closure executions')
                if child_files:
                    child_llvm = export('child', child_files)
                    child_rows = measure(manifest, child_llvm, crate, self.binary)
                    child_closure = next(row for row in child_rows if row['kind'] == 'closure')
                    self.assertGreater(sum(i['count'] for i in child_closure['instances']), 0,
                                       'original setsid callback must execute before child flush')
                    self.assertGreater(child_closure['lines']['covered'], 0)
                aggregate = export('combined', parent_files + child_files)
                rows = measure(manifest, aggregate, crate, self.binary)
                self.assertEqual(len(rows), len(inventory['symbols']))
                indices = []
                for row in rows:
                    self.assertGreater(row['lines']['count'], 0)
                    self.assertGreater(row['regions']['count'], 0)
                    indices.extend(i['index'] for i in row['instances'])
                    missing = copy.deepcopy(aggregate)
                    for index in sorted((i['index'] for i in row['instances']), reverse=True):
                        missing['data'][0]['functions'].pop(index)
                    with self.assertRaisesRegex(ValueError, re.escape('missing LLVM function: command.rs:' + row['name'] + ':')):
                        measure(manifest, missing, crate, self.binary)
                    negatives.append({'mode': mode, 'missing_owner': row['name'], 'observed_counts': [i['count'] for i in row['instances']]})
                self.assertEqual(len(indices), len(set(indices)), 'owners may not share LLVM records')
                closure = next(row for row in rows if row['kind'] == 'closure')
                owner = 'isolate_process_tree' if unix else 'terminate'
                parent = next(row for row in rows if row['name'] == owner)
                borrowed = copy.deepcopy(aggregate)
                for instance in closure['instances']:
                    fn = borrowed['data'][0]['functions'][instance['index']]
                    actual_parent = aggregate['data'][0]['functions'][parent['instances'][0]['index']]
                    fn['regions'], fn['filenames'] = copy.deepcopy(actual_parent['regions']), copy.deepcopy(actual_parent['filenames'])
                with self.assertRaisesRegex(ValueError, 'function kind mismatch: command.rs:' + owner):
                    measure(manifest, borrowed, crate, self.binary)
                negatives.append({'mode': mode, 'closure_cannot_borrow_parent': owner})
                if child_files:
                    missing = copy.deepcopy(child_llvm)
                    for index in sorted((i['index'] for i in child_closure['instances']), reverse=True):
                        missing['data'][0]['functions'].pop(index)
                    with self.assertRaisesRegex(ValueError, re.escape('missing LLVM function: command.rs:' + child_closure['name'] + ':')):
                        measure(manifest, missing, crate, self.binary)
                    negatives.append({'mode': mode, 'missing_child_closure': child_closure['name']})
                results[mode] = {'ordinary_parent': parent_rows, 'fixture_combined': rows,
                                 'child_flush': child_rows if child_files else None,
                                 'actual_driver_outcome': outcome,
                                 'legacy_eperm_permitted': darwin and label == 'base' and mode == 'isolated'}
                (crate / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
                (crate / 'negative-checks.json').write_text(json.dumps(negatives, indent=2) + '\n')
            if label == 'head' and 'target_os="macos"' in configuration['cfg']:
                for owner in ('observe_pid', 'list_process_group', 'darwin_group_is_finished'):
                    witnesses = [mode for mode, data in results.items()
                                 if any(row['name'] == owner and any(i['count'] > 0 for i in row['instances'])
                                        for row in data['ordinary_parent'])]
                    self.assertTrue(witnesses, 'new Darwin owner needs its own positive native counter: ' + owner)
            report['sources'][label] = {'source_sha256': digest(source.encode()), 'inventory': inventory,
                                       'binary_sha256': digest(executable.read_bytes()), 'results': results, 'validated': negatives}
        (evidence / 'native-result.json').write_text(json.dumps(report, indent=2) + '\n')
        hashes = {str(path.relative_to(evidence)): digest(path.read_bytes()) for path in evidence.rglob('*')
                  if path.is_file() and 'target' not in path.relative_to(evidence).parts}
        (evidence / 'sha256.json').write_text(json.dumps(hashes, indent=2) + '\n')

    def test_ast_controls_and_nested_ownership(self):
        source = '''const N: usize = 2;
fn outer(xs: &[bool]) -> bool {
    fn nested(x: bool) -> bool { if x { true } else { false } }
    let Some(x) = xs.first() else { return false };
    println!("{}", xs.iter().all(|x| *x && nested(*x)));
    match x { true if xs.len() > N => true, _ => false }
}
#[cfg(test)] mod tests { #[test] fn only_test() {} }
'''
        symbols = self.inventory(source)["symbols"]
        self.assertEqual([s["name"] for s in symbols], ["outer", "outer::nested", "outer::closure_5_34"])
        self.assertEqual([complexity(s["raw"]) for s in symbols], [4, 2, 2])
        self.assertEqual([s["test"] for s in symbols], [False, False, False])
        self.assertEqual(symbols[0]["raw"]["nested_functions"], 1)
        self.assertNotIn("and_and", symbols[0]["raw"])
        controls = self.inventory('''fn controls(mut xs: impl Iterator<Item=bool>) -> Option<()> {
    if xs.next()? && (true || false) {}
    if let Some(x) = xs.next() { let _ = x; }
    let Some(_) = xs.next() else { return None; };
    while xs.next().is_some() { break; }
    while let Some(_) = xs.next() { break; }
    for _ in 0..2 { loop { break; } }
    match xs.next() { Some(true) if true => (), Some(false) => (), _ => () }
    Some(())
}''')["symbols"][0]
        self.assertEqual(controls["raw"], {
            "if": 3, "question_mark": 1, "and_and": 1, "or_or": 1,
            "while": 2, "for": 1, "loop": 1, "match": 1,
            "match_arms": 3, "match_decisions": 2, "guards": 1,
        })
        self.assertEqual(complexity(controls["raw"]), 14)

    def test_vec_repeat_visits_value_length_and_closure_decisions(self):
        symbols = self.inventory('fn f(x: bool) { let _ = vec![|| if x { 1 } else { 2 }; if x { 2 } else { 1 }]; }')["symbols"]
        self.assertEqual([s["kind"] for s in symbols], ["function", "closure"])
        self.assertEqual([complexity(s["raw"]) for s in symbols], [2, 2])

    def test_unknown_macro_fails_closed(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.inventory("fn f() { custom!(name => |x| x); }")

    def test_empty_match_and_macro_decisions(self):
        symbols = self.inventory('fn f(x: !) { match x {} } fn g(x: bool) { println!("{}", if x { 1 } else { 2 }); }')["symbols"]
        self.assertEqual([complexity(s["raw"]) for s in symbols], [1, 2])

    def test_generic_closure_type_is_not_a_closure_function(self):
        self.assertFalse(closure_name("crate::run::<crate::parent::{closure#0}>"))
        self.assertTrue(closure_name("crate::run::<crate::parent::{closure#0}>::{closure#1}"))
        self.assertTrue(closure_name("crate::parent::{{closure}}"))

    def test_ratchet_preserves_uncovered_unchanged_closure_debt(self):
        rows = []
        for source, names in HOTSPOTS.items():
            for name in names:
                rows.append(dict(source=source, name=name, kind="function", syntax_sha256=digest(name.encode()),
                                 source_sha256="a" * 64, span=[1, 1, 1, 20], cc=1,
                                 crap_exact=[1, 1], passed=True))
        closure = dict(rows[0], name="run_check::closure_1_1", kind="closure", syntax_sha256="b" * 64,
                       passed=False, crap_exact=[2, 1])
        base = {"series": SERIES, "configuration": compiler_configuration(), "tools": {"fixture": "same"}, "functions": rows + [closure]}
        head = copy.deepcopy(base)
        head["functions"][-1]["name"] = "check_path::closure_2_1"
        head["functions"][-1]["span"] = [2, 1, 2, 20]
        result = compare(base, head)
        self.assertEqual(result["failures"], [])
        identity = result["identities"][-1]
        self.assertFalse(identity["changed"])
        self.assertTrue(identity["coverage_debt"])
        self.assertEqual(identity["base"]["name"], closure["name"])
        head["functions"][0]["passed"] = False
        self.assertEqual(len(compare(base, head)["failures"]), 1)
        head["series"] = dict(SERIES, mapping="different")
        with self.assertRaisesRegex(ValueError, "incompatible"):
            compare(base, head)
        head = copy.deepcopy(base)
        head["functions"].pop(1)
        with self.assertRaisesRegex(ValueError, "missing extracted hotspot"):
            compare(base, head)

    def test_production_outside_src_and_absent_base_are_explicit(self):
        with tempfile.TemporaryDirectory(dir=ROOT / "target") as temp:
            crate = Path(temp)
            (crate / "src").mkdir()
            (crate / "quality-core").mkdir()
            source = "pub fn evaluate() { let f = |x: bool| x; }"
            (crate / "quality-core/core.rs").write_bytes(source.encode())
            paths = ["../quality-core/core.rs", "app/quality.rs", "service/report_directory.rs"]
            manifest = prepare(crate, self.binary, paths)
            self.assertEqual(manifest["absent_sources"], ["app/quality.rs", "service/report_directory.rs"])
            self.assertNotIn("service/report_directory.rs", manifest["files"])
            self.assertEqual(manifest["files"][paths[0]]["original"], source)
            self.assertEqual(len(manifest["files"][paths[0]]["inventory"]["symbols"]), 2)
            transformed, _ = instrument(source, manifest["files"][paths[0]]["inventory"])
            self.assertEqual((crate / "quality-core/core.rs").read_bytes(), transformed.encode())
            with self.assertRaisesRegex(ValueError, "missing selected source"):
                prepare(crate, self.binary, ["app/commands.rs"])

    def test_unicode_nested_source_map(self):
        source = 'fn f() { let é = "é"; let f = |x| |y| x + y; }'
        inventory = self.inventory(source)
        transformed, edits = instrument(source, inventory)
        self.assertEqual(transformed.count("black_box"), 2)
        for token in ("x + y", "; }"):
            old = len(source[:source.index(token)].encode()) + 1
            new = len(transformed[:transformed.index(token)].encode()) + 1
            self.assertEqual(original_point([1, new], edits), (1, old))

    def test_instrumentation_distinguishes_unexecuted_closure_and_rejects_missing_evidence(self):
        source = '''#[cfg(unix)] fn unix_value() -> u8 { 1 }
#[cfg(windows)] fn windows_value() -> u8 { 2 }
struct Step { passed: bool }
#[inline(never)]
fn field_only(steps: &[Step]) -> bool {
    #[cfg(INACTIVE_HOST)] {
        let hidden = || if true { 10 } else { 20 };
        unsupported!(a => b);
    }
    steps.iter().all(|step| step.passed)
}
#[cfg(test)] mod tests { #[test] fn ignored() { unsupported!(a => b); } }
fn main() {
    #[cfg(unix)] { assert_eq!(unix_value(), 1); }
    #[cfg(windows)] { assert_eq!(windows_value(), 2); }
    let empty = std::env::args().nth(1).unwrap() == "empty";
    if empty { assert!(field_only(&[])); }
    else { assert!(!field_only(&[Step { passed: false }])); }
}
'''.replace('INACTIVE_HOST', 'unix' if 'windows' in compiler_configuration()['cfg'] else 'windows')
        with tempfile.TemporaryDirectory(dir=ROOT / "target") as temp:
            crate = Path(temp)
            (crate / "src").mkdir()
            (crate / "quality-core").mkdir()
            file = crate / "quality-core/fixture.rs"
            file.write_bytes(source.encode())
            inventory = ast(file, self.binary)
            self.assertEqual(len([s for s in inventory['symbols'] if s['kind'] == 'closure']), 1)
            transformed, edits = instrument(source, inventory)
            file.write_bytes(transformed.encode())
            manifest = {"series": SERIES, "configuration": compiler_configuration(), "files": {"../quality-core/fixture.rs": {
                "original": source, "original_sha256": digest(source.encode()),
                "instrumented_sha256": digest(transformed.encode()), "edits": edits, "inventory": inventory}}}
            executable = crate / ("fixture" + (".exe" if os.name == "nt" else ""))
            compile_command = ["rustc", "--edition=2021", "--target", manifest["configuration"]["target"],
                               "-C", "instrument-coverage", "-C", "opt-level=0", str(file), "-o", str(executable)]
            subprocess.run(compile_command, check=True, capture_output=True)
            sysroot = subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip()
            host = next(l.split(": ")[1] for l in subprocess.check_output(["rustc", "-vV"], text=True).splitlines() if l.startswith("host:"))
            llvm_bin = Path(sysroot) / "lib/rustlib" / host / "bin"
            suffix = ".exe" if os.name == "nt" else ""
            llvm_tools = {name: llvm_bin / (name + suffix) for name in ("llvm-profdata", "llvm-cov")}
            evidence = Path(os.environ["RUST_MEASURE_NATIVE_EVIDENCE"]) / 'cfg-closure' if os.environ.get("RUST_MEASURE_NATIVE_EVIDENCE") else None
            if evidence:
                evidence.mkdir(parents=True, exist_ok=False)
                (evidence / "instrumentation-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
                (evidence / "fixture.instrumented.rs").write_bytes(transformed.encode())
            results = {}
            for mode in ("empty", "executed"):
                raw, profile = crate / f"{mode}.profraw", crate / f"{mode}.profdata"
                subprocess.run([str(executable), mode], check=True, env={**os.environ, "LLVM_PROFILE_FILE": str(raw)})
                subprocess.run([str(llvm_tools["llvm-profdata"]), "merge", "-sparse", str(raw), "-o", str(profile)], check=True, capture_output=True)
                llvm = json.loads(subprocess.check_output([str(llvm_tools["llvm-cov"]), "export", str(executable), f"-instr-profile={profile}"]))
                if evidence:
                    shutil.copyfile(raw, evidence / raw.name)
                    shutil.copyfile(profile, evidence / profile.name)
                    (evidence / f"{mode}.llvm.json").write_text(json.dumps(llvm) + "\n")
                results[mode] = measure(manifest, llvm, crate, self.binary)
                active = 'windows_value' if 'windows' in manifest['configuration']['cfg'] else 'unix_value'
                inactive = 'unix_value' if active == 'windows_value' else 'windows_value'
                self.assertIn(active, {row['name'] for row in results[mode]})
                self.assertNotIn(inactive, {row['name'] for row in results[mode]})
                parent = next(row for row in results[mode] if row['name'] == 'field_only')
                self.assertLessEqual(parent['lines']['count'], 3, 'uncompiled body lines must leave the denominator')
            closures = {mode: next(s for s in rows if s["kind"] == "closure") for mode, rows in results.items()}
            self.assertEqual(closures["empty"]["instances"][0]["count"], 0)
            self.assertEqual(closures["empty"]["lines"]["covered"], 0)
            self.assertFalse(closures["empty"]["passed"])
            self.assertEqual(closures["executed"]["instances"][0]["count"], 1)
            self.assertTrue(closures["executed"]["passed"])
            missing = copy.deepcopy(llvm)
            missing["data"][0]["functions"].pop(closures["executed"]["instances"][0]["index"])
            with self.assertRaisesRegex(ValueError, "missing LLVM function"):
                measure(manifest, missing, crate, self.binary)
            missing_active = copy.deepcopy(llvm)
            active_row = next(row for row in results['executed'] if row['name'] == active)
            missing_active['data'][0]['functions'].pop(active_row['instances'][0]['index'])
            with self.assertRaisesRegex(ValueError, "missing LLVM function"):
                measure(manifest, missing_active, crate, self.binary)
            unsupported = copy.deepcopy(llvm)
            unsupported["data"][0]["functions"][closures["executed"]["instances"][0]["index"]]["regions"][0][7] = 1
            with self.assertRaisesRegex(ValueError, "unsupported macro expansion"):
                measure(manifest, unsupported, crate, self.binary)
            file.write_bytes((transformed + "\n// changed after collection\n").encode())
            with self.assertRaisesRegex(ValueError, "instrumented source mismatch"):
                measure(manifest, llvm, crate, self.binary)
            file.write_bytes(transformed.encode())
            manifest["files"]["../quality-core/fixture.rs"]["original_sha256"] = "0" * 64
            with self.assertRaisesRegex(ValueError, "original digest mismatch"):
                measure(manifest, llvm, crate, self.binary)
            if evidence:
                report = {"series": SERIES, "configuration": manifest["configuration"],
                          "checkout_sha": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                          "tools": provenance(self.binary), "compile_command": compile_command,
                          "fixture_binary_sha256": digest(executable.read_bytes()),
                          "test_source_sha256": digest(Path(__file__).read_bytes()),
                          "llvm_tools": {name: {"sha256": digest(path.read_bytes()),
                              "version": subprocess.check_output([str(path), "--version"], text=True).strip()}
                              for name, path in llvm_tools.items()},
                          "results": results,
                          "validated": ["inactive function and body exclusion", "zero-hit closure",
                                        "executed closure", "missing active function fails",
                                        "missing closure fails", "macro expansion fails",
                                        "changed instrumented bytes fail", "changed original digest fails"]}
                (evidence / "native-result.json").write_text(json.dumps(report, indent=2) + "\n")
                hashes = {path.name: digest(path.read_bytes()) for path in sorted(evidence.iterdir())}
                (evidence / "sha256.json").write_text(json.dumps(hashes, indent=2) + "\n")


class StepCacheRiskSelectionTests(unittest.TestCase):
    def test_cache_sources_are_additive_and_require_same_selection_recollection(self):
        required = {"verify/cache/mod.rs", "verify/cache/identity.rs", "verify/cache/store.rs",
                    "process/task.rs", "verify/scheduler.rs"}
        self.assertTrue(required <= set(SOURCE_FILES))
        self.assertEqual(SERIES['selection'], 'gh33-findings/1')
        self.assertTrue(set(HOTSPOTS) <= set(SOURCE_FILES))
        self.assertTrue({f"findings/{name}.rs" for name in ("mod", "model", "history", "sarif")} <= set(SOURCE_FILES))


class RetiredOperatorHooksSelectionTests(unittest.TestCase):
    def test_retired_hook_lifecycle_is_not_in_the_production_inventory(self):
        # Hook lifecycle moved to the verified external operator; Core keeps
        # hook execution/verify/compat but no longer owns lifecycle modules.
        for name in ('mod.rs', 'filesystem.rs', 'paths.rs'):
            self.assertNotIn('hooks/' + name, SOURCE_FILES)
        inventory = json.loads((ROOT / 'tools/quality/production-source.json').read_text(encoding='utf-8'))
        self.assertFalse(any('src/hooks/' in path
                             for paths in inventory['boundaries'].values()
                             for path in paths['files']))
        self.assertFalse((ROOT / 'tools/harness-gate/src/hooks').exists())
        self.assertNotIn('mod hooks;', (ROOT / 'tools/harness-gate/src/main.rs').read_text(encoding='utf-8'))


class RetiredSourceLinksSelectionTests(unittest.TestCase):
    def test_retired_source_links_is_not_in_the_production_inventory(self):
        # Platform navigation was retired: the GitHub-only module was deleted and
        # the risk series is otherwise unchanged.
        self.assertNotIn('source_links.rs', SOURCE_FILES)
        inventory = json.loads((ROOT / 'tools/quality/production-source.json').read_text(encoding='utf-8'))
        self.assertNotIn('src/source_links.rs', inventory['boundaries']['verify']['files'])
        self.assertFalse((ROOT / 'tools/harness-gate/src/source_links.rs').exists())
        self.assertNotIn('mod source_links;', (ROOT / 'tools/harness-gate/src/main.rs').read_text(encoding='utf-8'))
        self.assertEqual(inventory['threshold'], 80)
        self.assertEqual(SERIES['selection'], 'gh33-findings/1')


if __name__ == "__main__":
    unittest.main()
