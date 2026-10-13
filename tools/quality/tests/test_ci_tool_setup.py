"""Pinned setup must reject missing, failed, and incompatible tool evidence."""
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
ACTION = ROOT / '.github/actions/install-ci-tool'
PINS = {'cargo-nextest': '0.9.143', 'cargo-llvm-cov': '0.9.0',
        'cargo-audit': '0.22.2', 'cargo-tarpaulin': '0.37.5'}
INSTALLER = '183e4297cca2404691e9380e1307288dced5c82a'


class ToolSetupTests(unittest.TestCase):
    def invoke(self, mode, tool, output='', status=0):
        with tempfile.TemporaryDirectory() as temp:
            cargo = Path(temp) / 'cargo'
            cargo.write_text('#!/bin/sh\n'
                             '[ "$#" -eq 2 ] && [ "$1" = "$TEST_SUBCOMMAND" ] '
                             '&& [ "$2" = --version ] || exit 99\n'
                             'printf "%s\\n" "$TEST_VERSION"\nexit "$TEST_STATUS"\n')
            cargo.chmod(0o755)
            env = dict(os.environ, PATH=temp + os.pathsep + os.environ['PATH'],
                       TEST_VERSION=output, TEST_STATUS=str(status),
                       TEST_SUBCOMMAND=tool.removeprefix('cargo-'))
            return subprocess.run(['bash', str(ACTION / 'tool.sh'), mode, tool],
                                  env=env, capture_output=True, text=True)

    def test_explicit_pins_and_effective_versions(self):
        for tool, version in PINS.items():
            with self.subTest(tool=tool):
                resolved = self.invoke('resolve', tool)
                self.assertEqual(resolved.returncode, 0, resolved.stderr)
                self.assertEqual(resolved.stdout, f'spec={tool}@{version}\n')
                result = self.invoke('verify', tool, f'{tool} {version} (build info)\nrelease: {version}')
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(version, result.stdout)
        self.assertEqual(self.invoke('verify', 'cargo-audit', 'cargo-audit-audit 0.22.2').returncode, 0)
        self.assertEqual(self.invoke('verify', 'cargo-tarpaulin',
                                     'cargo-tarpaulin-tarpaulin 0.37.5').returncode, 0)

    def test_invalid_or_failed_version_evidence_blocks(self):
        for tool, version in PINS.items():
            for output, status in [('', 127), ('', 0), (f'{tool} 99.0.0', 0),
                                   (f'{tool} {version}-dev', 0),
                                   (f'wrong-tool {version}', 0),
                                   (f'{tool} {version}', 1)]:
                with self.subTest(tool=tool, output=output, status=status):
                    self.assertNotEqual(self.invoke('verify', tool, output, status).returncode, 0)

    def test_tarpaulin_effective_name_is_narrow_and_version_checked(self):
        for output, status in [('cargo-tarpaulin-tarpaulin 0.37.4', 0),
                               ('cargo-tarpaulin-tarpaulin 0.37.5-dev', 0),
                               ('cargo-tarpaulin-tarpaulin 0.37.5', 1),
                               ('cargo-tarpaulin-audit 0.37.5', 0),
                               ('cargo-audit-audit 0.37.5', 0),
                               ('tarpaulin 0.37.5', 0)]:
            with self.subTest(output=output, status=status):
                self.assertNotEqual(self.invoke('verify', 'cargo-tarpaulin',
                                                output, status).returncode, 0)

    def test_unknown_tool_or_operation_blocks(self):
        self.assertNotEqual(self.invoke('resolve', 'unknown').returncode, 0)
        self.assertNotEqual(self.invoke('unknown', 'cargo-nextest').returncode, 0)

    def test_workflows_require_verified_installation(self):
        action = (ACTION / 'action.yml').read_text()
        self.assertIn(f'uses: taiki-e/install-action@{INSTALLER}', action)
        self.assertIn("checksum: 'true'", action)
        self.assertIn('fallback: cargo-install', action)
        self.assertIn('steps.pin.outputs.spec', action)
        self.assertIn('tool.sh" verify "$CI_TOOL"', action)
        self.assertNotIn('continue-on-error', action)
        self.assertNotIn('if:', action)
        for name, count in [('ci.yml', 7), ('release.yml', 1), ('quality-baseline-refresh.yml', 1)]:
            workflow = (ROOT / '.github/workflows' / name).read_text()
            self.assertEqual(workflow.count('uses: ./.github/actions/install-ci-tool'), count)
            self.assertNotRegex(workflow, r'cargo install cargo-(nextest|llvm-cov|audit|tarpaulin)')
        ci = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('run: cargo audit --deny warnings', ci)
        self.assertIn('working-directory: tools/harness-gate', ci)
        self.assertNotIn('Cache cargo-audit', ci)
        release = (ROOT / '.github/workflows/release.yml').read_text()
        self.assertIn('cargo audit --file tools/harness-gate/Cargo.lock --deny warnings', release)

    def test_prebuilt_and_locked_fallback_share_required_verifier(self):
        # Freeze the reviewed upstream install/fallback contract. Neither route
        # may skip our Cargo-dispatched effective-version check after installation.
        action = (ACTION / 'action.yml').read_text()
        steps = re.split(r'^    - name: ', action, flags=re.M)[1:]
        self.assertEqual(len(steps), 3)
        self.assertIn('tool.sh" resolve "$CI_TOOL"', steps[0])
        self.assertIn(f'uses: taiki-e/install-action@{INSTALLER}', steps[1])
        self.assertIn('tool: ${{ steps.pin.outputs.spec }}', steps[1])
        self.assertIn("checksum: 'true'", steps[1])
        self.assertIn('fallback: cargo-install', steps[1])
        self.assertIn('tool.sh" verify "$CI_TOOL"', steps[2])
        for step in steps:
            if 'shell: bash' in step:
                self.assertIn('CI_TOOL: ${{ inputs.tool }}', step)
            self.assertNotRegex(step, r'\bif:|continue-on-error|\|\|\s*true')

    def test_tarpaulin_setup_preserves_coverage_execution(self):
        ci = (ROOT / '.github/workflows/ci.yml').read_text()
        body = ci.split('  coverage:\n', 1)[1].split('  quality-coverage:\n', 1)[0]
        self.assertIn("if: ${{ needs.classify.outputs.run_full == 'true' && github.event_name == 'push' }}", body)
        self.assertIn('runs-on: ubuntu-latest', body)
        self.assertIn('class: tarpaulin', body)
        self.assertIn("profiles: 'instrumented;default'", body)
        self.assertIn("cache-target: 'false'", body)
        self.assertIn('''      - name: Install tarpaulin
        uses: ./.github/actions/install-ci-tool
        with:
          tool: cargo-tarpaulin''', body)
        self.assertIn('run: cargo tarpaulin --manifest-path tools/harness-gate/Cargo.toml '
                      '--locked --engine llvm --timeout 300 --out Xml '
                      '--output-dir "$GITHUB_WORKSPACE/coverage"', body)
        self.assertIn('files: coverage/cobertura.xml', body)
        self.assertIn('fail_ci_if_error: false', body)
        self.assertNotIn('continue-on-error', body)

    def test_operating_docs_match_the_tool_contract(self):
        for name in ('README.md', 'tool-setup.md'):
            with self.subTest(document=name):
                document = (ROOT / 'docs/quality/ci-topology' / name).read_text()
                for tool, version in PINS.items():
                    self.assertIn(f'| {tool} | {version} |', document)
                self.assertIn(INSTALLER, document)
