#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Verify and stage the attributed, project-owned compatibility catalog data."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def catalog_paths(directory):
    languages = (directory / 'languages.txt').read_text(encoding='ascii').splitlines()
    if (len(languages) != 30 or languages != sorted(set(languages)) or
            any(not re.fullmatch(r'[a-z]{2}(?:_[A-Z]{2})?', language) for language in languages)):
        raise ValueError('invalid GNU Make 4.4.1 language inventory')
    return {language: f'{language}/LC_MESSAGES/make.mo' for language in languages}


def verify(directory):
    directory = directory.resolve()
    manifest = json.loads((directory / 'provenance.json').read_bytes())
    paths = catalog_paths(directory)
    required = {'COPYING', 'languages.txt', *paths.values(), *(f'{language}.po' for language in paths)}
    if set(manifest['files']) != required:
        raise ValueError('owned catalog/notice inventory differs')
    for name, expected in manifest['files'].items():
        path = directory / name
        if path.is_symlink() or not path.resolve().is_relative_to(directory):
            raise ValueError('catalog asset is outside its owned directory: ' + name)
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise ValueError('catalog asset hash differs: ' + name)
    header = manifest['translator_header'].encode()
    if not header or not (directory / 'fr.po').read_bytes().startswith(header):
        raise ValueError('catalog translator/package notice differs')
    return len(required)


def install(directory, destination):
    destination = destination.resolve()
    verify(directory)
    paths = catalog_paths(directory)
    for name in ('COPYING', 'provenance.json', 'languages.txt',
                 *(f'{language}.po' for language in paths), *paths.values()):
        target = destination / name
        if not target.parent.resolve().is_relative_to(destination) or target.is_symlink():
            raise ValueError('catalog destination escapes its installed directory: ' + name)
        target.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(dir=target.parent, prefix='.catalog-', delete=False) as stream:
            temporary = Path(stream.name)
            try:
                stream.write((directory / name).read_bytes())
                stream.flush()
                temporary.chmod(0o644)
                temporary.replace(target)
            finally:
                temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, default=ROOT / 'catalogs')
    parser.add_argument('--install', type=Path)
    arguments = parser.parse_args()
    try:
        if arguments.install is None:
            count = verify(arguments.directory)
            print(f'Verified {count} owned catalog and notice files.')
        else:
            install(arguments.directory, arguments.install)
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f'catalogs: {error}\n')


if __name__ == '__main__':
    main()
