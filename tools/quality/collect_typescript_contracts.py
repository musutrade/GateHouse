#!/usr/bin/env python3
"""Collect real task-3.3 contract runs in an isolated, disposable output directory."""
import argparse
import json
import os
from pathlib import Path
import selectors
import shutil
import subprocess
import urllib.request

from typescript_contracts import digest, inventory, write_json, OASDIFF_SHA256

FIXTURE = Path(__file__).parent / 'fixtures/typescript-angular'


def collect(output, oasdiff, scenario):
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    source = output / 'sources'
    for name in ('app', 'provider'):
        shutil.copytree(FIXTURE / name, source / name,
                        ignore=shutil.ignore_patterns('node_modules', '.angular', 'dist', 'coverage', 'target'))
    app, provider = source / 'app', source / 'provider'
    base = output / 'baseline.json'
    shutil.copyfile(provider / 'openapi.json', base)
    if scenario == 'breaking':
        spec = json.loads(base.read_text())
        spec['components']['schemas']['Quote']['required'].remove('currency')
        write_json(provider / 'openapi.json', spec)
    env = dict(os.environ, CI='true', NG_CLI_ANALYTICS='false',
               npm_config_cache=str(output.parent / 'npm-cache'),
               CARGO_TARGET_DIR=str(output / 'cargo-target'))
    manifest = dict(schema='typescript-contract-native/v1', scenario=scenario, status='failed',
                    revision=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                    commands=[], tools={}, provider='api', consumer='frontend')
    def save():
        write_json(output / 'manifest.json', manifest)
    def run(name, argv, cwd=source):
        entry = dict(name=name, argv=[str(a) for a in argv], cwd=str(cwd), exit_status=None,
                     env={key: env[key] for key in ('CI', 'NG_CLI_ANALYTICS', 'npm_config_cache',
                          'CARGO_TARGET_DIR', 'REFERENCE_PROVIDER_URL') if key in env},
                     stdout=name + '.stdout', stderr=name + '.stderr')
        manifest['commands'].append(entry)
        save()
        try:
            with (output / entry['stdout']).open('wb') as stdout, (output / entry['stderr']).open('wb') as stderr:
                result = subprocess.run(entry['argv'], cwd=cwd, env=env, stdout=stdout,
                                        stderr=stderr, timeout=600)
            entry['exit_status'] = result.returncode
            if result.returncode:
                raise RuntimeError(f'{name} exited {result.returncode}; see {output / entry["stderr"]}')
            return (output / entry['stdout']).read_text()
        finally:
            save()
    save()
    try:
        if digest(oasdiff) != OASDIFF_SHA256:
            raise ValueError('oasdiff executable digest mismatch (pinned Linux x86_64 release)')
        for name, argv in {'node': ['node', '--version'], 'npm': ['npm', '--version'],
                           'rustc': ['rustc', '--version'], 'cargo': ['cargo', '--version'],
                           'llvm-cov': ['cargo', 'llvm-cov', '--version'],
                           'oasdiff': [oasdiff, '--version']}.items():
            manifest['tools'][name] = run(name + '-version', argv).strip()
        for name, version in {'node': 'v24.18.0', 'npm': '11.16.0',
                              'oasdiff': 'oasdiff version 1.11.7',
                              'llvm-cov': 'cargo-llvm-cov 0.9.0'}.items():
            if manifest['tools'][name] != version:
                raise ValueError(f'unexpected {name} version')
        manifest['oasdiff_sha256'] = digest(oasdiff)
        run('npm-ci', ['npm', 'ci', '--no-audit', '--no-fund'], app)
        generator = app / 'node_modules/.bin/openapi'
        manifest['tools']['generator'] = run('generator-version', [generator, '--version'], app).strip()
        if manifest['tools']['generator'] != '0.29.0':
            raise ValueError('unexpected generator version')
        # Fresh baseline generation authenticates the checked-in consumer bytes.
        for label, contract in [('base-client', base), ('head-client', provider / 'openapi.json')]:
            run(label, [generator, '--input', contract, '--output', output / label, '--client', 'fetch'], app)
        if inventory(output / 'base-client') != inventory(app / 'src/app/generated'):
            raise ValueError('consumer bytes differ from fresh baseline generation')
        run('oasdiff', [oasdiff, 'breaking', base, provider / 'openapi.json', '--format', 'json'])
        run('rust-coverage', ['cargo', 'llvm-cov', '--locked', '--json', '--output-path',
                             output / 'rust-coverage.json'], provider)
        run('rust-build', ['cargo', 'build', '--locked'], provider)
        run('angular-build', ['npm', 'run', 'build'], app)
        server_argv = [str(output / 'cargo-target/debug/angular-reference-provider')]
        with (output / 'provider.stderr').open('wb') as stderr:
            server = subprocess.Popen(server_argv, stdout=subprocess.PIPE, stderr=stderr, env=env)
            try:
                with selectors.DefaultSelector() as selector:
                    selector.register(server.stdout, selectors.EVENT_READ)
                    if not selector.select(timeout=15):
                        raise RuntimeError('provider startup timeout')
                url = server.stdout.readline().decode().strip()
                manifest['server'] = dict(argv=server_argv, url=url)
                env['REFERENCE_PROVIDER_URL'] = url
                with urllib.request.urlopen(url + '/openapi.json', timeout=10) as response:
                    (output / 'served-openapi.json').write_bytes(response.read())
                if digest(output / 'served-openapi.json') != digest(provider / 'openapi.json'):
                    raise ValueError('served contract differs from measured contract')
                run('angular-test', ['npm', 'exec', '--', 'ng', 'test', '--watch=false', '--coverage'], app)
                if server.poll() is not None:
                    raise RuntimeError('provider exited during tests')
            finally:
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait()
                server.stdout.close()
                manifest['server_exit'] = server.returncode
        coverage = list((app / 'coverage').rglob('coverage-final.json'))
        if len(coverage) != 1:
            raise ValueError('missing or ambiguous Angular coverage')
        shutil.copyfile(coverage[0], output / 'angular-coverage.json')
        manifest['status'] = 'measured'
    finally:
        # Retain original inputs, generated bytes and raw output; exclude tool caches.
        for name in ('node_modules', '.angular', 'dist', 'coverage'):
            shutil.rmtree(app / name, ignore_errors=True)
        shutil.rmtree(output / 'cargo-target', ignore_errors=True)
        save()
        write_json(output / 'receipt.json', dict(schema='typescript-contract-receipt/v1',
                   files=inventory(output, exclude={'receipt.json'})))
    return output


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--oasdiff', type=Path, required=True)
    parser.add_argument('--scenario', choices=['compatible', 'breaking'], required=True)
    args = parser.parse_args()
    collect(args.output, args.oasdiff.resolve(), args.scenario)
