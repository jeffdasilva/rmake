#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Authenticate pinned Ubuntu locale data and generate a private French locale.

No package is installed, no maintainer script runs, and no system locale or
archive is changed. The generated output is accepted only at its locked hash.
"""
from __future__ import annotations

import argparse
import fcntl
import gzip
import hashlib
import io
import json
import lzma
import os
from pathlib import Path, PurePosixPath
import platform
import re
import stat
import subprocess
import sys
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / 'oracle/locale-data.json'
ARCHIVE = 'https://archive.ubuntu.com/ubuntu/'
MAX_BYTES = 96 * 1024 * 1024


class ProfileError(RuntimeError):
    pass


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def relative(name: str) -> Path:
    value = PurePosixPath(name)
    if not name or value.is_absolute() or '..' in value.parts or '\0' in name or name == '.':
        raise ProfileError('locale profile path must be relative')
    return Path(value)


def no_symlinks(path: Path) -> None:
    for parent in [*path.parents, path]:
        if parent.is_symlink():
            raise ProfileError('locale profile path is a symlink: ' + str(parent))


def inventory(root: Path) -> list[dict]:
    rows = []; total = 0; links = {}
    for path in sorted(root.rglob('*'), key=lambda p: os.fsencode(p.relative_to(root))):
        info = path.lstat(); row = {'path': str(path.relative_to(root)), 'mode': stat.S_IMODE(info.st_mode)}
        if stat.S_ISDIR(info.st_mode):
            row['type'] = 'directory'
        elif stat.S_ISLNK(info.st_mode):
            row.update(type='symlink', target=os.readlink(path))
        elif stat.S_ISREG(info.st_mode):
            total += info.st_size
            if total > 128 * 1024 * 1024:
                raise ProfileError('locale profile exceeds byte bound')
            row.update(type='file', size=info.st_size, sha256=digest(path.read_bytes()))
            links.setdefault((info.st_dev, info.st_ino), []).append(row)
        else:
            raise ProfileError('locale profile contains a special file')
        rows.append(row)
        if len(rows) > 20000:
            raise ProfileError('locale profile exceeds entry bound')
    for group in links.values():
        if len(group) > 1:
            names = sorted(row['path'] for row in group)
            for row in group: row['hardlinks'] = names
    return rows


def tree_digest(root: Path) -> str:
    return digest((json.dumps(inventory(root), indent=2) + '\n').encode())


class HttpsOnly(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, newurl):
        if not newurl.startswith('https://'):
            raise ProfileError('locale profile download redirects outside HTTPS')
        return super().redirect_request(request, fp, code, message, headers, newurl)


def download(path: Path, url: str, expected: str, size: int | None = None) -> bytes:
    if not url.startswith('https://') or not re.fullmatch(r'[a-f0-9]{64}', expected):
        raise ProfileError('invalid pinned locale download')
    limit = MAX_BYTES if size is None else size
    if not 0 < limit <= MAX_BYTES or path.is_symlink():
        raise ProfileError('invalid locale download bound or symlink cache')
    if path.exists():
        if path.stat().st_size > limit:
            raise ProfileError('cached locale download exceeds size bound')
        data = path.read_bytes()
    else:
        with urllib.request.build_opener(HttpsOnly()).open(url, timeout=30) as response:
            data = response.read(limit + 1)
    if len(data) > limit or (size is not None and len(data) != size) or digest(data) != expected:
        raise ProfileError('locale download/cache size or digest differs: ' + path.name)
    if not path.exists():
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix='.verified-', delete=False) as stream:
            stream.write(data); temporary = Path(stream.name)
        temporary.replace(path)
    return data


def write_owned(path: Path, content: bytes) -> None:
    """Replace cache metadata without following an existing target symlink."""
    no_symlinks(path)
    with tempfile.NamedTemporaryFile(dir=path.parent, prefix='.owned-', delete=False) as stream:
        stream.write(content); temporary = Path(stream.name)
    temporary.replace(path)


def release_hashes(data: bytes) -> dict[str, tuple[str, int]]:
    lines = data.decode('utf-8').splitlines()
    try: start = lines.index('SHA256:') + 1
    except ValueError as error: raise ProfileError('signed release lacks SHA256 section') from error
    result = {}
    for line in lines[start:]:
        if not line.startswith(' '): break
        fields = line.split()
        if len(fields) != 3 or not re.fullmatch(r'[a-f0-9]{64}', fields[0]):
            raise ProfileError('invalid signed SHA256 record')
        relative(fields[2])
        if fields[2] in result or not fields[1].isdigit():
            raise ProfileError('duplicate or malformed signed SHA256 record')
        result[fields[2]] = (fields[0], int(fields[1]))
    return result


def package_stanzas(data: bytes):
    for paragraph in data.decode('utf-8').split('\n\n'):
        fields = {}
        for line in paragraph.splitlines():
            if line and not line[0].isspace() and ': ' in line:
                key, value = line.split(': ', 1)
                if key in fields: raise ProfileError('duplicate package metadata field')
                fields[key] = value
        if fields: yield fields, paragraph + '\n'


def run(command: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(command, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=kwargs.pop('timeout', 60), **kwargs)


def authenticate(lock: dict, destination: Path) -> dict:
    provenance = destination / 'provenance'; provenance.mkdir(exist_ok=True)
    no_symlinks(provenance)
    release = download(provenance / 'noble-InRelease', lock['release_url'], lock['release_sha256'])
    home = destination / 'gnupg'; home.mkdir(mode=0o700, exist_ok=True); no_symlinks(home)
    signature = run(['gpgv', '--homedir', str(home), '--status-fd', '1', '--keyring',
                     lock['keyring'], str(provenance / 'noble-InRelease')], env={'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'})
    valid = b'[GNUPG:] VALIDSIG ' + lock['signing_fingerprint'].encode() + b' '
    if valid not in signature.stdout:
        raise ProfileError('release lacks the pinned Ubuntu signing identity')
    write_owned(provenance / 'signature-status.txt', signature.stdout + signature.stderr)
    hashes = release_hashes(release); index = lock['signed_index']; name = index['path']
    if hashes.get(name) != (index['sha256'], index['size']) or index['size'] > MAX_BYTES:
        raise ProfileError('locked locale index differs from signed release')
    cache = provenance / 'main-Packages.gz'; no_symlinks(cache)
    if cache.exists():
        with gzip.open(cache, 'rb') as stream: data = stream.read(index['size'] + 1)
    else:
        packed_name = name + '.xz'
        if packed_name not in hashes: raise ProfileError('signed compressed index is absent')
        expected, size = hashes[packed_name]
        packed = download(provenance / 'main-Packages.xz', ARCHIVE + 'dists/noble/' + packed_name, expected, size)
        with lzma.LZMAFile(io.BytesIO(packed)) as stream: data = stream.read(index['size'] + 1)
    if len(data) != index['size'] or digest(data) != index['sha256']:
        raise ProfileError('locale index fails signed size/digest')
    if not cache.exists(): write_owned(cache, gzip.compress(data, mtime=0))
    records = {row['name']: row for row in lock['packages']}
    if set(records) != {'locales', 'libc-bin'} or len(lock['packages']) != 2:
        raise ProfileError('locale profile requires exactly two locked packages')
    found = {}
    for fields, paragraph in package_stanzas(data):
        name = fields.get('Package')
        if name not in records: continue
        row = records[name]
        expected = {'Package': row['name'], 'Version': row['version'], 'Architecture': row['architecture'],
                    'SHA256': row['sha256'], 'Size': str(row['size']), 'Filename': row['url'].removeprefix(ARCHIVE)}
        if name in found or any(fields.get(key) != value for key, value in expected.items()):
            raise ProfileError('package differs from authenticated locale index: ' + name)
        found[name] = paragraph
    if set(found) != set(records): raise ProfileError('locale package absent from signed index')
    for name, paragraph in found.items(): write_owned(provenance / (name + '.package'), paragraph.encode())
    return {'release_sha256': digest(release), 'index_sha256': digest(data),
            'signature_status_hex': signature.stdout.hex(),
            'keyring_sha256': digest(Path(lock['keyring']).read_bytes())}


def validate_inputs(lock: dict, prefix: Path) -> dict[str, str]:
    result = {}
    for name, expected in lock['input_files'].items():
        path = prefix / relative(name); no_symlinks(path)
        if not path.is_file() or digest(path.read_bytes()) != expected:
            raise ProfileError('locale input differs: ' + name)
        result[name] = expected
    return result


def generate(lock: dict, prefix: Path, locale_root: Path, work: Path) -> dict:
    locale_root.mkdir()
    charmap = work / 'UTF-8'
    with gzip.open(prefix / 'usr/share/i18n/charmaps/UTF-8.gz', 'rb') as stream:
        data = stream.read(16 * 1024 * 1024 + 1)
    if len(data) > 16 * 1024 * 1024 or digest(data) != lock['charmap_uncompressed_sha256']:
        raise ProfileError('decompressed UTF-8 charmap differs')
    charmap.write_bytes(data)
    command = [str(prefix / 'usr/bin/localedef'), '--no-archive', '-i',
               str(prefix / 'usr/share/i18n/locales/fr_FR'), '-f', str(charmap), str(locale_root / lock['locale'])]
    environment = {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'LANG': 'C', 'TZ': 'UTC',
                   'HOME': str(work), 'I18NPATH': str(prefix / 'usr/share/i18n')}
    result = run(command, env=environment, umask=0o022)
    if result.stdout or result.stderr: raise ProfileError('localedef emitted unexpected diagnostic bytes')
    return {'command': command, 'environment': environment, 'stdout_hex': result.stdout.hex(), 'stderr_hex': result.stderr.hex()}


def provision_locked(destination: Path) -> dict:
    if platform.machine() != 'x86_64' or platform.libc_ver() != ('glibc', '2.39'):
        raise ProfileError('pinned locale profile requires Linux/x86-64 glibc2.39')
    lock_bytes = LOCK.read_bytes(); lock = json.loads(lock_bytes)
    if lock.get('schema_version') != 1 or lock['locale'] != 'fr_FR.UTF-8':
        raise ProfileError('unsupported locale data lock')
    for name in ('installed_tree_sha256', 'generated_tree_sha256', 'charmap_uncompressed_sha256'):
        if not re.fullmatch('[a-f0-9]{64}', lock.get(name, '')):
            raise ProfileError('locale lock lacks pinned content: ' + name)
    authentication = authenticate(lock, destination)
    packages = destination / 'packages'; packages.mkdir(exist_ok=True); no_symlinks(packages)
    archives = [packages / row['url'].rsplit('/', 1)[1] for row in lock['packages']]
    for archive, row in zip(archives, lock['packages']): download(archive, row['url'], row['sha256'], row['size'])
    install = destination / 'installs' / digest(lock_bytes); no_symlinks(install); install.mkdir(parents=True, exist_ok=True)
    prefix = install / 'prefix'; locale_root = install / 'locales'; no_symlinks(prefix); no_symlinks(locale_root)
    if prefix.exists():
        if tree_digest(prefix) != lock['installed_tree_sha256']: raise ProfileError('existing locale input tree differs')
    else:
        with tempfile.TemporaryDirectory(prefix='extract-', dir=install) as temporary:
            extracted = Path(temporary) / 'prefix'; extracted.mkdir()
            for archive in archives: run(['dpkg-deb', '-x', str(archive), str(extracted)])
            validate_inputs(lock, extracted)
            if tree_digest(extracted) != lock['installed_tree_sha256']: raise ProfileError('fresh locale input tree differs')
            extracted.replace(prefix)
    inputs = validate_inputs(lock, prefix)
    if locale_root.exists():
        if tree_digest(locale_root) != lock['generated_tree_sha256']: raise ProfileError('existing compiled locale differs')
        generation = {'reused_verified_output': True}
    else:
        with tempfile.TemporaryDirectory(prefix='generate-', dir=install) as temporary:
            work = Path(temporary); output = work / 'locales'
            generation = generate(lock, prefix, output, work)
            if tree_digest(output) != lock['generated_tree_sha256']: raise ProfileError('fresh compiled locale differs')
            output.replace(locale_root)
    smoke_environment = {'PATH': '/usr/bin:/bin', 'LC_ALL': lock['locale'], 'LANG': lock['locale'],
                         'TZ': 'UTC', 'LOCPATH': str(locale_root)}
    smoke = run([sys.executable, '-B', '-c', 'import locale;print(locale.setlocale(locale.LC_ALL,""))'], env=smoke_environment)
    if smoke.stdout != (lock['locale'] + '\n').encode() or smoke.stderr:
        raise ProfileError('private compiled locale is not usable by host libc')
    profile = {'schema_version': 1, 'classification': lock['classification'], 'lock_sha256': digest(lock_bytes),
               'locale': lock['locale'], 'environment': {'LOCPATH': str(locale_root)},
               'authentication': authentication, 'packages': lock['packages'], 'input_files': inputs,
               'installed_tree_sha256': lock['installed_tree_sha256'], 'generated_tree_sha256': lock['generated_tree_sha256'],
               'generated_files': inventory(locale_root), 'generation': generation,
               'smoke_stdout_hex': smoke.stdout.hex(), 'smoke_stderr_hex': smoke.stderr.hex(),
               'host': {'machine': platform.machine(), 'libc': platform.libc_ver()},
               'no_system_install': True, 'maintainer_scripts_run': False}
    write_owned(destination / 'profile.json', (json.dumps(profile, indent=2) + '\n').encode())
    return profile


def provision(destination: Path) -> dict:
    destination = Path(destination).absolute()
    no_symlinks(destination)
    if (destination / 'fr_FR.UTF-8').is_dir():
        raise ProfileError('locale profile root points to compiled LOCPATH')
    destination.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(destination / '.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'a') as stream:
        fcntl.flock(stream, fcntl.LOCK_EX)
        return provision_locked(destination)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT / '.tools/locale-data')
    args = parser.parse_args(argv)
    try: profile = provision(args.root)
    except (ProfileError, OSError, ValueError, subprocess.SubprocessError) as error:
        print('ERROR locale data: ' + str(error), file=sys.stderr); return 1
    print(json.dumps(profile, indent=2)); return 0


if __name__ == '__main__':
    raise SystemExit(main())
