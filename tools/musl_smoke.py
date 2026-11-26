#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Recreate and exercise the selected x86-64 dynamic musl profile on Ubuntu."""

from __future__ import annotations

import argparse
from dataclasses import replace
import functools
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import subprocess
import tempfile
import urllib.request

import conformance
import native_modules
import oracle
import process_restarts
import restart_pipe
import rust

TARGET = 'x86_64-unknown-linux-musl'
MAKE_HOST = 'x86_64-pc-linux-musl'
MUSL_VERSION = '1.2.4-2'
SNAPSHOT = 'https://snapshot.ubuntu.com/ubuntu/20250401T000000Z/pool/universe/m/musl'
# Noble signed archive metadata, both amd64 packages from the release pocket.
MUSL_PACKAGES = {
    'musl': (415772, '9f0883c20b4b746e05e947bafd99cb933f5494ffaaa6fcd360cbe1fbcf264883'),
    'musl-dev': (615590, '4b451ecb6a0f8469883058cf22a807f3bd9cc16d115cc08b7efc35fe8eb44db2'),
}
UCONTEXT_COMMIT = '49e671dd52ff6791295d8161ad3b6da7dc5f6f9d'
UCONTEXT_SHA256 = 'c84f57298ed4bce278c31ed8e5c0b3da6f9bd8bc45c9dfb08656fe797bdc2c47'
UNWIND_SHA256 = '9adff5e5a4add5249918c9c759a5905f98d966b78abc943ce01efdc1a33533c4'
ORDINARY_CASES = (
    'include-serial-forced-named-dry-cleaner-clock-retimed-present',
    'shell-status-lexical-foreach-retains-status',
    'wildcard-class', 'wildcard-invalid-class',
    'wildcard-negated-class', 'wildcard-posix-alpha', 'wildcard-posix-space',
    'wildcard-quoted-hyphen-class',
    'wildcard-quoted-name-literal',
    'wildcard-simple-brackets-posix-empty', 'wildcard-simple-brackets-posix-set',
    'wildcard-simple-brackets-posix-unset',
)
NATIVE_CASES = {'native-setup-result-negative-one',
                'native-setup-result-negative-one-optional-explicit-and-repeat'}
RESTART_CASES = {'process-restart-tempdir', 'process-restart-replaced-image'}
SCOPE = ('x86-64 Linux using a private musl 1.2.4 dynamic loader, Guile and NLS disabled, '
         'C locale and the Ubuntu host /bin/sh')


def fetch(path: Path, url: str, sha256: str, size_limit: int) -> None:
    opener = urllib.request.build_opener(oracle.HTTPSRedirectsOnly())

    def attempt():
        with opener.open(url, timeout=30) as response:
            return response.read(size_limit + 1)

    data = oracle.retry_download(attempt)
    if len(data) > size_limit or hashlib.sha256(data).hexdigest() != sha256:
        raise oracle.OracleError(f'pinned download size/digest mismatch: {url}')
    path.write_bytes(data)


def executable(path: Path, text: str) -> Path:
    path.write_text(text, encoding='utf-8')
    path.chmod(0o700)
    return path


def build_sysroot(root, env):
    sysroot = root / 'sysroot'
    sources = {}
    for package, (size, digest) in MUSL_PACKAGES.items():
        name = f'{package}_{MUSL_VERSION}_amd64.deb'
        url = f'{SNAPSHOT}/{name}'
        fetch(root / name, url, digest, size)
        subprocess.run(['/usr/bin/dpkg-deb', '-x', root / name, sysroot], env=env, check=True)
        sources[package] = {'url': url, 'sha256': digest}
    loader = sysroot / 'lib/x86_64-linux-musl/libc.so'
    spec = (sysroot / 'usr/lib/x86_64-linux-musl/musl-gcc.specs').read_text()
    spec = (spec.replace('/usr/include/x86_64-linux-musl', str(sysroot / 'usr/include/x86_64-linux-musl'))
            .replace('/usr/lib/x86_64-linux-musl', str(sysroot / 'usr/lib/x86_64-linux-musl'))
            .replace('/lib/ld-musl-x86_64.so.1', str(loader)))
    specs = root / 'musl-gcc.specs'
    specs.write_text(spec, encoding='utf-8')
    compiler = executable(root / 'cc',
                          f'#!/bin/sh\nexec /usr/bin/gcc -specs {shlex.quote(str(specs))} '
                          f'-L{shlex.quote(str(loader.parent))} "$@"\n')
    return loader, compiler, sources


def build_ucontext(root, compiler, loader, env):
    name = f'libucontext-{UCONTEXT_COMMIT}'
    url = f'https://codeload.github.com/kaniini/libucontext/tar.gz/{UCONTEXT_COMMIT}'
    archive = root / f'{name}.tar.gz'
    fetch(archive, url, UCONTEXT_SHA256, 1024 * 1024)
    library_dir = oracle.safe_extract(archive, root / 'ucontext-source', name)
    subprocess.run(['/usr/bin/make', '-j2', 'ARCH=x86_64', 'BUILD_POSIX=yes', 'EXPORT_UNPREFIXED=yes',
                    f'CC={compiler}', 'CFLAGS=-O2 -g0 -Wall', f'LDFLAGS=-Wl,-rpath,{loader.parent}'],
                   cwd=library_dir, env=env, check=True)
    (library_dir / 'libucontext_posix.so.1').symlink_to('libucontext_posix.so')
    return library_dir, {'url': url, 'sha256': UCONTEXT_SHA256, 'commit': UCONTEXT_COMMIT}


def build_reference(root, config, compiler, loader, env):
    archive = oracle.download_archive(config, root / 'downloads')
    source = oracle.safe_extract(archive, root / 'gnu-source', config['archive']['root'])
    build = root / 'gnu-build'
    prefix = root / 'oracle'
    build.mkdir()
    build_env = {**env, 'CC': str(compiler), 'CFLAGS': config['cflags'],
                 'LDFLAGS': f'-Wl,-rpath,{loader.parent}'}
    subprocess.run([source / 'configure', *config['configure_args'], f'--prefix={prefix}'],
                   cwd=build, env=build_env, check=True)
    config_header = (build / 'src/config.h').read_text()
    if '#define USE_POSIX_SPAWN 1\n' not in config_header or '/* #undef ENABLE_NLS */' not in config_header:
        raise oracle.OracleError('unexpected GNU POSIX spawn or NLS configuration')
    subprocess.run(['/usr/bin/make', '-j2'], cwd=build, env=build_env, check=True)
    (prefix / 'bin').mkdir(parents=True)
    (prefix / 'include').mkdir()
    reference = prefix / 'bin/make'
    shutil.copy2(build / 'make', reference)
    shutil.copy2(source / 'src/gnumake.h', prefix / 'include/gnumake.h')
    return reference


def build_candidate(root, compiler, loader, ucontext, reference, env):
    rust_root = rust.local_tool_root(rust.X86_64_HOST)
    rust_bin = rust_root / 'cargo/bin'
    cargo_env = {**env, 'CARGO_HOME': str(rust_root / 'cargo'), 'RUSTUP_HOME': str(rust_root / 'rustup'),
                 'RUSTUP_TOOLCHAIN': rust.TOOLCHAIN, 'PATH': f'{rust_bin}:{env["PATH"]}'}
    subprocess.run([rust_bin / 'rustup', 'target', 'add', '--toolchain', rust.TOOLCHAIN, TARGET],
                   env=cargo_env, check=True)
    sysroot = Path(oracle.checked_run([str(rust_bin / 'rustc'), '--print', 'sysroot'],
                                     cwd=oracle.ROOT, env=cargo_env).strip())
    unwind = sysroot / f'lib/rustlib/{TARGET}/lib/self-contained/libunwind.a'
    oracle.verify_archive(unwind, UNWIND_SHA256)
    linker = executable(root / 'rust-linker',
                        f'#!/bin/sh\nexec {shlex.quote(str(compiler))} -L{shlex.quote(str(root))} "$@"\n')
    # Rust supplies a musl-built static unwinder. Use this linker input in place
    # of the host GCC's glibc-dependent libgcc_s when Rust links dynamically.
    (root / 'libunwind.a').symlink_to(unwind)
    (root / 'libgcc_s.so').write_text(f"INPUT({root / 'libunwind.a'})\n", encoding='utf-8')
    flags = ['-C', 'target-feature=-crt-static', '-C', f'link-arg=-Wl,-rpath,{loader.parent}',
             '-C', f'link-arg=-Wl,-rpath,{ucontext}']
    build_env = {**cargo_env, 'CARGO_TARGET_DIR': str(root / 'target'), 'CARGO_INCREMENTAL': '0',
                 'CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER': str(linker),
                 'CC_x86_64_unknown_linux_musl': str(compiler), 'RMAKE_UCONTEXT_LIBDIR': str(ucontext),
                 'RMAKE_INCLUDEDIR': str(reference.parent.parent / 'include'),
                 'CARGO_ENCODED_RUSTFLAGS': '\x1f'.join(flags)}
    subprocess.run([rust_bin / 'cargo', 'build', '--locked', '-j2', '--release', '--target', TARGET, '-p', 'rmake'],
                   cwd=oracle.ROOT, env=build_env, check=True)
    return root / f'target/{TARGET}/release/rmake'


def observe_runtime(binary, *, loader, libraries):
    env = oracle.clean_environment()
    elf = oracle.checked_run(['/usr/bin/readelf', '-l', '-d', str(binary)], cwd=loader.parent, env=env)
    if f'[Requesting program interpreter: {loader}]' not in elf:
        raise oracle.OracleError(f'unexpected ELF interpreter for {binary}')
    listing = oracle.checked_run([str(loader), '--list', str(binary)], cwd=loader.parent, env=env)
    paths = {Path(name) for name in re.findall(r'(?:^\s*|=>\s+)(/\S+)\s+\(', listing, re.M)}
    if paths != {loader, *libraries} or 'not found' in listing:
        raise oracle.OracleError(f'unexpected musl runtime dependencies: {listing.strip()}')
    shell = Path('/bin/sh')
    return {
        'system': platform.system(), 'architecture': platform.machine(), 'kernel': platform.release(),
        'os_release_sha256': oracle.sha256_file(Path('/etc/os-release')),
        'libc': ['musl', MUSL_VERSION],
        'interpreter': str(loader), 'needed': re.findall(r'\(NEEDED\).*?\[([^\]]+)\]', elf),
        'libraries': {str(path): oracle.sha256_file(path) for path in sorted(paths)},
        'shell': {'path': str(shell), 'resolved_path': str(shell.resolve()), 'sha256': oracle.sha256_file(shell)},
        'observation_environment': env,
    }


def utf8_wildcard_case(root, reference, candidate):
    case = conformance.load_case(oracle.ROOT / 'tests/musl/wildcard-utf8-byte-width')
    case = replace(case, environment={**case.environment, 'LC_ALL': 'C.UTF-8'})
    root.mkdir()
    first, second = (conformance.execute(case, root, reference) for _ in range(2))
    mismatches = (
        conformance.expectation_errors(case, first, root=root)
        + conformance.expectation_errors(case, second, root=root)
        + conformance.differences(first, second)
    )
    gnu2_qualified = not mismatches
    observations = {'gnu-1': first, 'gnu-2': second}
    if gnu2_qualified:
        trial = conformance.execute(case, root, candidate)
        observations['candidate'] = trial
        mismatches.extend(conformance.differences(first, trial))
    print('FAIL' if mismatches else 'PASS', case.id, ', '.join(mismatches), flush=True)
    return {
        'id': case.id, 'environment': case.environment, 'expected': case.expected,
        'gnu2_qualified': gnu2_qualified, 'passed': not mismatches, 'mismatches': mismatches,
        'observations': {
            name: {**conformance.observation_json(value), 'stdout_hex': value.stdout.hex(),
                   'stderr_hex': value.stderr.hex()}
            for name, value in observations.items()
        },
    }


def cases(root, reference, candidate, compiler):
    ordinary = []
    workspace = root / 'ordinary'
    workspace.mkdir()
    for name in ORDINARY_CASES:
        case = conformance.load_case(oracle.ROOT / 'tests/cases' / name)
        first, second, trial = [conformance.execute(case, workspace, binary)
                                for binary in (reference, reference, candidate)]
        mismatches = (
            conformance.expectation_errors(case, first, root=workspace)
            + conformance.expectation_errors(case, second, root=workspace)
            + conformance.differences(first, second)
            + conformance.differences(first, trial)
        )
        if any(run.limit or run.leftover_processes for run in (first, second, trial)):
            mismatches.append('process limit')
        ordinary.append({'id': name, 'mismatches': mismatches})
        print('FAIL' if mismatches else 'PASS', name, ', '.join(mismatches), flush=True)

    utf8_wildcard = utf8_wildcard_case(root / 'utf8-wildcard', reference, candidate)

    native = [model for model in native_modules.models() if model.id in NATIVE_CASES]
    restarts = [model for model in process_restarts.models() if model['id'] in RESTART_CASES]
    if {model.id for model in native} != NATIVE_CASES or {model['id'] for model in restarts} != RESTART_CASES:
        raise oracle.OracleError('a selected musl smoke model is unavailable')
    native_root = root / 'native'
    native_root.mkdir()
    native_report = native_modules.run_campaign(native, reference, candidate, str(compiler), native_root)
    restart_root = root / 'restart'
    restart_root.mkdir()
    restart_report = process_restarts.run(restarts, reference, candidate, None, restart_root)
    pipe = restart_pipe.read_model(restart_pipe.MODEL)
    if pipe.id != 'restart-inherited-pipe-success':
        raise oracle.OracleError(f'unexpected inherited pipe smoke model: {pipe.id}')
    host_path = os.environ.get('PATH')
    try:
        # The pipe fixture resolves cc before launching its clean observations.
        os.environ['PATH'] = f'{root}:/usr/bin:/bin'
        pipe_report = restart_pipe.run(pipe, [reference, reference, candidate])
    finally:
        if host_path is None:
            os.environ.pop('PATH', None)
        else:
            os.environ['PATH'] = host_path
    passed = (all(not case['mismatches'] for case in ordinary) and utf8_wildcard['passed']
              and native_report['passed'] and restart_report['passed'] and pipe_report['passed'])
    return {'passed': passed, 'ordinary': ordinary, 'utf8_wildcard': utf8_wildcard,
            'native': native_report, 'process_restarts': restart_report, 'restart_pipe': pipe_report}


def run(root, report, revision):
    env = oracle.clean_environment()
    loader, compiler, sources = build_sysroot(root, env)
    ucontext, sources['libucontext'] = build_ucontext(root, compiler, loader, env)
    config = oracle.load_config()
    config['profile_scope'] = SCOPE
    reference = build_reference(root, config, compiler, loader, env)
    candidate = build_candidate(root, compiler, loader, ucontext, reference, env)
    reference_runtime = functools.partial(observe_runtime, loader=loader, libraries=())
    libraries = tuple(ucontext / name for name in ('libucontext_posix.so.1', 'libucontext.so.1'))
    observed = observe_runtime(candidate, loader=loader, libraries=libraries)
    build = {'sources': sources, 'rust_version': rust.TOOLCHAIN, 'rust_unwind_sha256': UNWIND_SHA256,
             'compiler': oracle.checked_run(['/usr/bin/gcc', '--version'], cwd=root, env=env).splitlines()[0],
             'linker': oracle.checked_run(['/usr/bin/ld', '--version'], cwd=root, env=env).splitlines()[0]}
    profile = oracle.create_profile(reference, config, build, runtime_observer=reference_runtime)
    profile_path = root / 'oracle/profile.json'
    profile_path.write_text(json.dumps(profile, indent=2) + '\n', encoding='utf-8')
    oracle.verify(reference, profile_path, config, runtime_observer=reference_runtime)
    for name, variables in (('reference', profile['observed']['variables']),
                            ('candidate', oracle.probe_variables(candidate))):
        if variables['MAKE_HOST'] != MAKE_HOST or variables['.INCLUDE_DIRS'] != profile['observed']['variables']['.INCLUDE_DIRS']:
            raise oracle.OracleError(f'unexpected {name} host metadata or include directory')
    digest = oracle.sha256_file(candidate)
    observations = cases(root, reference, candidate, compiler)
    oracle.verify(reference, profile_path, config, runtime_observer=reference_runtime)
    if oracle.sha256_file(candidate) != digest or observe_runtime(candidate, loader=loader, libraries=libraries) != observed:
        raise oracle.OracleError('musl candidate or runtime changed during observation')
    report.parent.mkdir(parents=True, exist_ok=True)
    result = {'reference': profile,
              'candidate': {'revision': revision, 'sha256': digest, 'runtime': observed},
              'cases': observations}
    report.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    return observations['passed']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--revision', default=os.environ.get('GITHUB_SHA'))
    args = parser.parse_args()
    if (platform.system(), platform.machine()) != ('Linux', 'x86_64'):
        parser.error('the musl smoke profile requires x86-64 Linux')
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-musl-') as directory:
            root = Path(directory).resolve()
            # The GCC specs and linker arguments embed this exact directory.
            if not re.fullmatch(r'/[a-zA-Z0-9_./-]+', str(root)):
                raise oracle.OracleError('the musl work directory must have a simple absolute path')
            return 0 if run(root, args.report.resolve(), args.revision) else 1
    except (OSError, subprocess.CalledProcessError, oracle.OracleError, conformance.HarnessError) as error:
        parser.exit(1, f'musl smoke: {error}\n')


if __name__ == '__main__':
    raise SystemExit(main())
