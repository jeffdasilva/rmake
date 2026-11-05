#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Pinned locally extracted Guile profile; no system install or GNU fallback."""
from __future__ import annotations

import argparse
import fcntl
import gzip
import hashlib
import json
import lzma
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shlex
import shutil
import stat
import subprocess
import sys
import tempfile
import urllib.request

import oracle

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / 'oracle/guile/toolchain.json'
MAX_INDEX = 96 * 1024 * 1024


class ProfileError(RuntimeError):
    pass


def digest(data):
    return hashlib.sha256(data).hexdigest()


def tree_inventory(prefix):
    """Path-independent bytes/types/modes, without following package symlinks."""
    rows = []
    links = {}
    total = 0
    for path in sorted(prefix.rglob('*'), key=lambda p: os.fsencode(p.relative_to(prefix))):
        metadata = path.lstat()
        row = {'path': str(path.relative_to(prefix)), 'mode': stat.S_IMODE(metadata.st_mode)}
        if stat.S_ISLNK(metadata.st_mode):
            row.update(type='symlink', target=os.readlink(path))
        elif stat.S_ISDIR(metadata.st_mode):
            row['type'] = 'directory'
        elif stat.S_ISREG(metadata.st_mode):
            total += metadata.st_size
            if total > 512 * 1024 * 1024:
                raise ProfileError('extracted profile exceeds byte bound')
            row.update(type='file', size=metadata.st_size, sha256=digest(path.read_bytes()))
            links.setdefault((metadata.st_dev, metadata.st_ino), []).append(row)
        else:
            raise ProfileError('unexpected special file in extracted profile')
        rows.append(row)
        if len(rows) > 20000:
            raise ProfileError('extracted profile exceeds entry bound')
    for group in links.values():
        if len(group) > 1:
            paths = sorted(row['path'] for row in group)
            for row in group: row['hardlinks'] = paths
    return rows


def tree_digest(prefix):
    return digest((json.dumps(tree_inventory(prefix), indent=2) + '\n').encode())


def prefix_for(destination):
    return destination / 'installs' / digest(LOCK.read_bytes()) / 'prefix'


def run(command, *, timeout=60, **kwargs):
    return subprocess.run(command, check=True, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, timeout=timeout, **kwargs)


class HttpsOnly(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, newurl):
        if not newurl.startswith('https://'):
            raise ProfileError('profile download redirects outside HTTPS')
        return super().redirect_request(request, fp, code, message, headers, newurl)


def cached_download(path, url, expected, size):
    if not url.startswith('https://') or size < 0 or size > MAX_INDEX:
        raise ProfileError('invalid pinned profile download')
    if path.is_symlink():
        raise ProfileError('profile cache entry is a symlink')
    if path.exists():
        data = path.read_bytes()
    else:
        opener = urllib.request.build_opener(HttpsOnly())
        with opener.open(url, timeout=30) as response:
            data = response.read(size + 1)
    if len(data) != size or digest(data) != expected:
        raise ProfileError('profile download/cache size or digest differs: ' + path.name)
    if not path.exists():
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix='.verified-', delete=False) as output:
            temporary = Path(output.name); output.write(data)
        temporary.replace(path)
    return data


def release_hashes(data):
    lines = data.decode('utf-8').splitlines()
    try:
        start = lines.index('SHA256:') + 1
    except ValueError as error:
        raise ProfileError('signed release lacks SHA256 section') from error
    result = {}
    for line in lines[start:]:
        if not line.startswith(' '):
            break
        fields = line.split()
        if len(fields) != 3 or not re.fullmatch(r'[0-9a-f]{64}', fields[0]):
            raise ProfileError('malformed signed SHA256 record')
        name = PurePosixPath(fields[2])
        if name.is_absolute() or '..' in name.parts or fields[2] in result:
            raise ProfileError('invalid signed index path')
        result[fields[2]] = (fields[0], int(fields[1]))
    return result


def package_stanzas(data):
    for paragraph in data.decode('utf-8').split('\n\n'):
        fields = {}
        for line in paragraph.splitlines():
            if line and not line[0].isspace() and ': ' in line:
                key, value = line.split(': ', 1)
                if key in fields:
                    raise ProfileError('duplicate package metadata field')
                fields[key] = value
        if fields:
            yield fields, paragraph + '\n'


def validate_package(record, fields):
    expected = {'Package': record['name'], 'Version': record['version'],
                'Architecture': record['architecture'], 'SHA256': record['sha256'],
                'Size': str(record['size']),
                'Filename': record['url'].removeprefix('https://archive.ubuntu.com/ubuntu/')}
    if any(fields.get(key) != value for key, value in expected.items()):
        raise ProfileError('locked package differs from authenticated index: ' + record['name'])


def authenticate(lock, destination):
    provenance = destination / 'provenance'; provenance.mkdir(exist_ok=True)
    release = provenance / 'noble-InRelease'
    if release.is_symlink():
        raise ProfileError('cached noble release is a symlink')
    if not release.exists():
        # Size is intentionally discovered only for the fixed digest-checked
        # release. Index/package lengths come from signed metadata and the lock.
        opener = urllib.request.build_opener(HttpsOnly())
        with opener.open(lock['release_url'], timeout=30) as response:
            data = response.read(1024 * 1024 + 1)
        if len(data) > 1024 * 1024 or digest(data) != lock['release_sha256']:
            raise ProfileError('pinned noble release differs')
        release.write_bytes(data)
    if release.is_symlink() or digest(release.read_bytes()) != lock['release_sha256']:
        raise ProfileError('cached noble release differs')
    home = destination / 'gnupg'; home.mkdir(mode=0o700, exist_ok=True)
    signature = run(['gpgv', '--homedir', str(home), '--status-fd', '1',
                     '--keyring', lock['keyring'], str(release)])
    if b'[GNUPG:] VALIDSIG ' not in signature.stdout:
        raise ProfileError('noble release has no verified signature')
    (provenance / 'signature-status.txt').write_bytes(signature.stdout + signature.stderr)
    hashes = release_hashes(release.read_bytes())
    records = {row['name']: row for row in lock['packages']}
    if len(records) != 17 or len(records) != len(lock['packages']):
        raise ProfileError('unexpected pinned package closure')
    found = {}
    for index in lock['signed_indices']:
        relative = index['path']
        if hashes.get(relative) != (index['sha256'], index['size']) or index['size'] > MAX_INDEX:
            raise ProfileError('locked index differs from signed release')
        raw_cache = provenance / (relative.split('/')[0] + '-Packages.gz')
        if raw_cache.is_symlink():
            raise ProfileError('index cache is a symlink')
        if raw_cache.exists():
            with gzip.open(raw_cache, 'rb') as stream:
                data = stream.read(index['size'] + 1)
        else:
            packed_name = relative + '.xz'
            identity, size = hashes[packed_name]
            packed = provenance / (relative.split('/')[0] + '-Packages.xz')
            cached_download(packed, 'https://archive.ubuntu.com/ubuntu/dists/noble/' + packed_name, identity, size)
            with lzma.open(packed, 'rb') as stream:
                data = stream.read(index['size'] + 1)
        if len(data) != index['size'] or digest(data) != index['sha256']:
            raise ProfileError('index fails signed uncompressed size/digest')
        if not raw_cache.exists():
            raw_cache.write_bytes(gzip.compress(data, mtime=0))
        for fields, paragraph in package_stanzas(data):
            name = fields.get('Package')
            if name in records and fields.get('Architecture') in ('amd64', 'all'):
                if name in found or records[name]['signed_index'] != relative:
                    raise ProfileError('ambiguous locked package')
                validate_package(records[name], fields)
                found[name] = paragraph
    if set(found) != set(records):
        raise ProfileError('locked packages missing from authenticated indices')
    for name, paragraph in found.items():
        (provenance / (name + '.package')).write_text(paragraph)
    return {'release_sha256': lock['release_sha256'],
            'keyring_sha256': digest(Path(lock['keyring']).read_bytes()),
            'signature_status_hex': signature.stdout.hex()}


def environment(destination):
    prefix = prefix_for(destination); library = prefix / 'usr/lib/x86_64-linux-gnu'
    return {'PATH': str(destination / 'bin') + ':/usr/bin:/bin', 'LC_ALL': 'C', 'LANG': 'C',
            'TZ': 'UTC', 'HOME': str(destination / 'home'), 'CONFIG_SITE': '/dev/null',
            'RMAKE_GUILE_PREFIX': str(prefix), 'LD_LIBRARY_PATH': str(library),
            'GUILE_LOAD_PATH': str(prefix / 'usr/share/guile/3.0'),
            'GUILE_LOAD_COMPILED_PATH': str(library / 'guile/3.0/ccache'), 'GUILE_AUTO_COMPILE': '0'}


def wrappers(destination):
    prefix = prefix_for(destination); library = prefix / 'usr/lib/x86_64-linux-gnu'
    binary = destination / 'bin'; binary.mkdir(exist_ok=True)
    for name, target in (('guile', 'guile-3.0'), ('pkg-config', 'pkgconf')):
        script = '#!/bin/sh\nset -eu\n'
        for key, value in environment(destination).items():
            if key != 'PATH': script += 'export ' + key + '=' + shlex.quote(value) + '\n'
        if name == 'pkg-config':
            script += 'export PKG_CONFIG_SYSROOT_DIR=' + shlex.quote(str(prefix)) + '\n'
            script += 'export PKG_CONFIG_LIBDIR=' + shlex.quote(str(library / 'pkgconfig') + ':' + str(prefix / 'usr/share/pkgconfig')) + '\n'
        script += 'exec ' + shlex.quote(str(prefix / 'usr/bin' / target)) + ' "$@"\n'
        path = binary / name; path.write_text(script); path.chmod(0o755)


def provision_locked(destination):
    if platform.machine() != 'x86_64' or platform.libc_ver() != ('glibc', '2.39'):
        raise ProfileError('this pinned tool profile requires x86_64 glibc2.39')
    lock = json.loads(LOCK.read_text())
    if lock.get('schema_version') != 1:
        raise ProfileError('unknown Guile tool lock')
    if not re.fullmatch(r'[0-9a-f]{64}', lock.get('installed_tree_sha256', '')):
        raise ProfileError('tool lock lacks a pinned installed tree')
    destination.mkdir(parents=True, exist_ok=True)
    (destination / 'home').mkdir(exist_ok=True)
    packages = destination / 'packages'; packages.mkdir(exist_ok=True)
    authentication = authenticate(lock, destination)
    archives = []
    for record in lock['packages']:
        archive = packages / record['url'].rsplit('/', 1)[1]
        cached_download(archive, record['url'], record['sha256'], record['size'])
        archives.append(archive)
    prefix = prefix_for(destination)
    if any(path.is_symlink() for path in (destination / 'installs', prefix.parent, prefix)):
        raise ProfileError('profile install path is a symlink')
    prefix.parent.mkdir(parents=True, exist_ok=True)
    if prefix.exists():
        if tree_digest(prefix) != lock['installed_tree_sha256']:
            raise ProfileError('existing immutable profile tree differs')
    else:
        with tempfile.TemporaryDirectory(prefix='extract-', dir=prefix.parent) as temporary:
            extracted = Path(temporary) / 'prefix'; extracted.mkdir()
            for archive in archives:
                run(['dpkg-deb', '-x', str(archive), str(extracted)])
            for record in lock['packages']:
                relative = Path(record['license_file']).relative_to('prefix')
                if digest((extracted / relative).read_bytes()) != record['license_sha256']:
                    raise ProfileError('extracted package license differs')
            if tree_digest(extracted) != lock['installed_tree_sha256']:
                raise ProfileError('fresh extracted profile tree differs')
            extracted.replace(prefix)
    wrappers(destination)
    probe = run([str(destination / 'bin/guile'), '--no-auto-compile', '-c', '(display (+ 20 22))(newline)'],
                cwd=destination, env=environment(destination), timeout=20)
    if probe.stdout != b'42\n' or probe.stderr:
        raise ProfileError('extracted Guile runtime capability differs')
    profile = {'schema_version': 1, 'classification': lock['classification'],
               'lock_sha256': digest(LOCK.read_bytes()), 'authentication': authentication,
               'environment': environment(destination), 'packages': lock['packages'],
               'smoke_stdout_hex': probe.stdout.hex(), 'no_system_install': True, 'maintainer_scripts_run': False}
    profile['installed_tree_sha256'] = lock['installed_tree_sha256']
    (destination / 'profile.json').write_text(json.dumps(profile, indent=2) + '\n')
    print('Verified local Guile3.0.9/GC8.2.6 profile:', destination)
    return profile


def provision(destination):
    destination.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(destination / '.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'a') as stream:
        fcntl.flock(stream, fcntl.LOCK_EX)
        return provision_locked(destination)


def build_reference(destination):
    """Build the fixed GNU source solely as a separate comparison executable."""
    configuration = json.loads((ROOT / 'oracle/manifest.json').read_text())
    source_identity = configuration['archive']['sha256']
    archive = ROOT / '.tools/downloads/make-4.4.1.tar.gz'
    if not archive.is_file() or digest(archive.read_bytes()) != source_identity:
        raise ProfileError('bootstrap the pinned disabled oracle archive first')
    compiler = Path('/usr/bin/cc').resolve()
    inputs = {'source_sha256': source_identity, 'tool_lock_sha256': digest(LOCK.read_bytes()),
              'compiler_sha256': digest(compiler.read_bytes()), 'destination': str(destination)}
    identity = digest(json.dumps(inputs, sort_keys=True).encode())
    parent = destination / 'references'; parent.mkdir(exist_ok=True)
    prefix = parent / identity
    profile_path = parent / (identity + '.json')
    if profile_path.exists():
        profile = json.loads(profile_path.read_text())
        if profile['inputs'] != inputs:
            raise ProfileError('cached enabled-reference inputs differ')
        for item in [profile['binary'], *profile['runtime_dependencies']]:
            path = Path(item['path'])
            if not path.is_file() or digest(path.read_bytes()) != item['sha256']:
                raise ProfileError('cached enabled-reference runtime differs')
        return profile
    if prefix.exists():
        # Preserve an earlier incomplete install rather than accepting it.
        failed = Path(tempfile.mkdtemp(prefix='failed-reference-', dir=parent))
        prefix.rename(failed / 'partial-install')
    prefix.mkdir()
    env = environment(destination)
    package_prefix = prefix_for(destination)
    library = package_prefix / 'usr/lib/x86_64-linux-gnu'
    env.update({'CFLAGS': '-O2 -g0',
                'CPPFLAGS': shlex.join(['-I' + str(package_prefix / 'usr/include'),
                                       '-I' + str(package_prefix / 'usr/include/x86_64-linux-gnu')]),
                'LDFLAGS': shlex.join(['-Wl,-rpath,' + str(library)]),
                'PKG_CONFIG': str(destination / 'bin/pkg-config')})
    log_path = parent / (identity + '-build.log')
    # A fresh extraction prevents old configure/object state from silently
    # retaining the disabled profile despite a successful rebuild.
    with tempfile.TemporaryDirectory(prefix='gnu-enabled-build-', dir=destination) as tmp:
        source = oracle.safe_extract(archive, Path(tmp), 'make-4.4.1')
        commands = [[str(source / 'configure'), '--prefix=' + str(prefix), '--with-guile',
                     '--enable-nls', '--disable-dependency-tracking'],
                    ['/usr/bin/make', '-j2'], ['/usr/bin/make', 'install']]
        with log_path.open('wb') as log:
            for command in commands:
                subprocess.run(command, cwd=source, env=env, check=True,
                               stdout=log, stderr=subprocess.STDOUT, timeout=300)
    binary = prefix / 'bin/make'
    body = b'$(info FEATURES=[$(.FEATURES)])\n$(info RESULT=[$(guile (+ 20 22))])\nall:;@:\n'
    with tempfile.TemporaryDirectory(prefix='enabled-smoke-', dir=destination) as tmp:
        result = run([str(binary), '-rR', '--no-print-directory', '-f', '-'],
                     cwd=tmp, env=env, input=body, timeout=10)
    lines = result.stdout.splitlines()
    if result.stderr or len(lines) != 2 or not lines[0].startswith(b'FEATURES=[') or b'guile' not in lines[0][10:-1].split() or lines[1] != b'RESULT=[42]':
        raise ProfileError('enabled GNU reference capability did not qualify')
    dependencies = run(['/usr/bin/ldd', str(binary)], env=env, timeout=10)
    runtime = []
    for line in dependencies.stdout.decode().splitlines():
        paths = [word for word in line.split() if word.startswith('/')]
        if paths:
            path = Path(paths[0]).resolve()
            runtime.append({'path': str(path), 'sha256': digest(path.read_bytes())})
    profile = {'schema_version': 1, 'classification': 'Separate Guile-enabled GNU4.4.1 comparison profile; zero rmake implementation/compatibility credit',
               'inputs': inputs, 'environment': env, 'commands': commands,
               'binary': {'path': str(binary), 'sha256': digest(binary.read_bytes())},
               'runtime_dependencies': runtime, 'smoke_input_hex': body.hex(),
               'smoke_stdout_hex': result.stdout.hex(), 'smoke_stderr_hex': result.stderr.hex(),
               'build_log_sha256': digest(log_path.read_bytes())}
    profile_path.write_text(json.dumps(profile, indent=2) + '\n')
    (destination / 'reference-profile.json').write_bytes(profile_path.read_bytes())
    print('Verified separate Guile-enabled GNU4.4.1 reference:', binary)
    return profile


def component_tests(destination):
    env = {**os.environ, **environment(destination),
           'CARGO_TARGET_DIR': str(ROOT / 'target/guile-component')}
    env.pop('RMAKE_GUILE_COMPONENT_CASE', None)
    # This opt-in process-isolated component suite does not activate Guile in
    # the rmake executable or claim GNU-compatible Scheme conversion/lifecycle.
    subprocess.run([sys.executable, str(ROOT / 'tools/rust.py'), 'cargo', 'test',
                    '--locked', '-p', 'rmake-linux', '--features', 'guile',
                    '--test', 'guile_dispatcher'], cwd=ROOT, env=env, check=True, timeout=300)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT / '.tools/guile')
    parser.add_argument('--reference', action='store_true')
    parser.add_argument('--environment', action='store_true')
    parser.add_argument('--components', action='store_true')
    args = parser.parse_args(argv)
    destination = args.root.resolve()
    provision(destination)
    if args.reference:
        descriptor = os.open(destination / '.reference-lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
        with os.fdopen(descriptor, 'a') as stream:
            fcntl.flock(stream, fcntl.LOCK_EX)
            build_reference(destination)
    if args.components:
        component_tests(destination)
    if args.environment:
        print(json.dumps(environment(destination), indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
