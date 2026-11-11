# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Negative controls for authenticated, private locale provisioning."""
import gzip
import io
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import unittest
import urllib.error
from unittest.mock import call, patch

import locale_data as data


class LocaleDataTests(unittest.TestCase):
    def test_lock_pins_packages_inputs_signer_and_entire_generated_output(self):
        lock = json.loads(data.LOCK.read_bytes())
        self.assertEqual(lock['locale'], 'fr_FR.UTF-8')
        self.assertEqual({row['name'] for row in lock['packages']}, {'locales', 'libc-bin'})
        self.assertEqual({row['version'] for row in lock['packages']}, {'2.39-0ubuntu8'})
        self.assertEqual(lock['signing_fingerprint'], 'F6ECB3762474EDA9D21B7022871920D1991BC93C')
        for name in ('installed_tree_sha256', 'generated_tree_sha256', 'charmap_uncompressed_sha256'):
            self.assertRegex(lock[name], '^[a-f0-9]{64}$')
        self.assertIn('usr/bin/localedef', lock['input_files'])
        self.assertIn('usr/share/i18n/locales/fr_FR', lock['input_files'])
        for package in ('locales', 'libc-bin'):
            self.assertIn('usr/share/doc/' + package + '/copyright', lock['input_files'])

    def test_download_rejects_changed_bytes_size_scheme_and_symlink_cache(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); path = root / 'cache'; path.write_bytes(b'good')
            self.assertEqual(data.download(path, 'https://example.invalid/input', data.digest(b'good'), 4), b'good')
            for url, sha, size in (('http://example.invalid', data.digest(b'good'), 4),
                                   ('https://example.invalid', data.digest(b'bad'), 4),
                                   ('https://example.invalid', data.digest(b'good'), 3)):
                with self.assertRaises(data.ProfileError): data.download(path, url, sha, size)
            link = root / 'link'; link.symlink_to(path)
            with self.assertRaises(data.ProfileError): data.download(link, 'https://example.invalid', data.digest(b'good'), 4)
            with self.assertRaises(data.ProfileError): data.write_owned(link, b'bad')
            self.assertEqual(path.read_bytes(), b'good')
            with self.assertRaises(data.ProfileError):
                data.HttpsOnly().redirect_request(None, None, None, None, None, 'http://example.invalid')

    def test_archive_fallback_caches_only_the_pinned_locale_data(self):
        path = 'pool/test.deb'
        primary = data.ARCHIVE + path
        alternate = data.oracle.UBUNTU_ARCHIVE_ALTERNATE + path
        unresolved = urllib.error.URLError(socket.gaierror(socket.EAI_NONAME, 'unknown host'))
        for failover, payload in ((False, b'bad!'), (True, b'bad!'), (True, b'good')):
            with (self.subTest(failover=failover, payload=payload),
                  tempfile.TemporaryDirectory() as temporary,
                  patch.object(data.urllib.request, 'build_opener') as opener):
                cache = Path(temporary) / 'package.deb'
                download = opener.return_value.open
                download.side_effect = [unresolved, io.BytesIO(payload)] if failover else [io.BytesIO(payload)]
                if payload == b'good':
                    self.assertEqual(data.download(cache, primary, data.digest(b'good'), 4), payload)
                    self.assertEqual(cache.read_bytes(), payload)
                else:
                    with self.assertRaisesRegex(data.ProfileError, 'size or digest differs'):
                        data.download(cache, primary, data.digest(b'good'), 4)
                    self.assertEqual(list(cache.parent.iterdir()), [])
                endpoints = [primary, alternate] if failover else [primary]
                self.assertEqual(download.call_args_list, [call(url, timeout=30) for url in endpoints])

    def test_signed_metadata_rejects_duplicate_fields_and_unsafe_paths(self):
        good = ('Header\nSHA256:\n ' + 'a' * 64 + ' 42 main/binary-amd64/Packages\nNext:\n').encode()
        self.assertEqual(data.release_hashes(good), {'main/binary-amd64/Packages': ('a' * 64, 42)})
        for raw in (b'no hashes', good.replace(b'main/binary-amd64/Packages', b'../outside'),
                    good.replace(b'Next:', b' ' + b'b' * 64 + b' 1 main/binary-amd64/Packages')):
            with self.assertRaises(data.ProfileError): data.release_hashes(raw)
        with self.assertRaises(data.ProfileError): list(data.package_stanzas(b'Package: one\nPackage: two\n'))
        for name in ('/absolute', '../outside', 'inside/../../outside', '\0bad', '.'):
            with self.assertRaises(data.ProfileError): data.relative(name)

    def test_authentication_requires_signature_index_and_both_exact_package_records(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); (root / 'provenance').mkdir(); keyring = root / 'keyring'; keyring.write_bytes(b'keyring')
            rows = [{'name': name, 'version': '2.39-0ubuntu8', 'architecture': arch,
                     'url': data.ARCHIVE + name + '.deb', 'sha256': str(index) * 64, 'size': index}
                    for index, (name, arch) in enumerate((('locales', 'all'), ('libc-bin', 'amd64')), 1)]
            def paragraph(row):
                return '\n'.join(f'{key}: {value}' for key, value in {
                    'Package': row['name'], 'Version': row['version'], 'Architecture': row['architecture'],
                    'SHA256': row['sha256'], 'Size': row['size'], 'Filename': row['url'].removeprefix(data.ARCHIVE)}.items())
            index = ('\n\n'.join(paragraph(row) for row in rows) + '\n\n').encode()
            (root / 'provenance/main-Packages.gz').write_bytes(gzip.compress(index))
            index_name = 'main/binary-amd64/Packages'
            release = f'SHA256:\n {data.digest(index)} {len(index)} {index_name}\n'.encode()
            lock = {'release_url': 'https://example.invalid/InRelease', 'release_sha256': data.digest(release),
                    'signing_fingerprint': 'PINNED', 'keyring': str(keyring),
                    'signed_index': {'path': index_name, 'sha256': data.digest(index), 'size': len(index)}, 'packages': rows}
            with patch.object(data, 'download', return_value=release), \
                 patch.object(data, 'run', return_value=subprocess.CompletedProcess([], 0, b'[GNUPG:] VALIDSIG PINNED rest\n', b'')):
                result = data.authenticate(lock, root)
                self.assertEqual(result['index_sha256'], data.digest(index))
                wrong = {**lock, 'packages': [{**rows[0], 'version': 'wrong'}, rows[1]]}
                with self.assertRaisesRegex(data.ProfileError, 'differs from authenticated'): data.authenticate(wrong, root)
                with self.assertRaisesRegex(data.ProfileError, 'differs from signed'):
                    data.authenticate({**lock, 'signed_index': {**lock['signed_index'], 'sha256': '0' * 64}}, root)
            with patch.object(data, 'download', return_value=release), \
                 patch.object(data, 'run', return_value=subprocess.CompletedProcess([], 0, b'[GNUPG:] VALIDSIG OTHER rest\n', b'')):
                with self.assertRaisesRegex(data.ProfileError, 'pinned Ubuntu'): data.authenticate(lock, root)

    def test_inventory_detects_changes_modes_hardlinks_symlinks_and_special_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); path = root / 'input'; path.write_bytes(b'original')
            first = data.tree_digest(root); path.write_bytes(b'changed')
            self.assertNotEqual(data.tree_digest(root), first)
            second = data.tree_digest(root); path.chmod(0o600)
            self.assertNotEqual(data.tree_digest(root), second)
            (root / 'alias').hardlink_to(path)
            self.assertEqual(data.inventory(root)[0]['hardlinks'], ['alias', 'input'])
            (root / 'link').symlink_to('input')
            with self.assertRaises(data.ProfileError): data.no_symlinks(root / 'link' / 'child')
            import os
            os.mkfifo(root / 'fifo')
            with self.assertRaisesRegex(data.ProfileError, 'special file'): data.inventory(root)

    def test_generation_uses_packaged_localedef_private_inputs_and_explicit_charmap(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); prefix = root / 'prefix'; work = root / 'work'; work.mkdir()
            charmap = prefix / 'usr/share/i18n/charmaps/UTF-8.gz'; charmap.parent.mkdir(parents=True)
            charmap.write_bytes(gzip.compress(b'original map'))
            lock = {'locale': 'fr_FR.UTF-8', 'charmap_uncompressed_sha256': data.digest(b'original map')}
            with patch.object(data, 'run', return_value=subprocess.CompletedProcess([], 0, b'', b'')) as run:
                generated = data.generate(lock, prefix, root / 'locales', work)
            command = generated['command']
            self.assertEqual(command[0], str(prefix / 'usr/bin/localedef'))
            self.assertIn('--no-archive', command)
            self.assertEqual(command[command.index('-i') + 1], str(prefix / 'usr/share/i18n/locales/fr_FR'))
            self.assertEqual((work / 'UTF-8').read_bytes(), b'original map')
            self.assertEqual(run.call_args.kwargs['env']['I18NPATH'], str(prefix / 'usr/share/i18n'))
            self.assertEqual(run.call_args.kwargs['umask'], 0o022)
            with patch.object(data, 'run', return_value=subprocess.CompletedProcess([], 0, b'', b'warning')):
                with self.assertRaisesRegex(data.ProfileError, 'diagnostic'):
                    data.generate(lock, prefix, root / 'other', work)
            with self.assertRaisesRegex(data.ProfileError, 'charmap differs'):
                data.generate({**lock, 'charmap_uncompressed_sha256': '0' * 64}, prefix, root / 'bad', work)

    def test_cached_profile_verifies_all_input_and_output_bytes_before_smoke(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); prefix = root / 'expected-input'; prefix.mkdir(); (prefix / 'localedef').write_bytes(b'original')
            output = root / 'expected-output'; output.mkdir(); (output / 'LC_COLLATE').write_bytes(b'compiled')
            lock = {'schema_version': 1, 'locale': 'fr_FR.UTF-8', 'classification': 'test',
                    'installed_tree_sha256': data.tree_digest(prefix), 'generated_tree_sha256': data.tree_digest(output),
                    'charmap_uncompressed_sha256': '0' * 64, 'input_files': {'localedef': data.digest(b'original')},
                    'packages': [{'url': 'https://example.invalid/one.deb', 'sha256': '0' * 64, 'size': 1}]}
            lock_path = root / 'lock.json'; lock_path.write_text(json.dumps(lock)); destination = root / 'profile'
            install = destination / 'installs' / data.digest(lock_path.read_bytes()); install.mkdir(parents=True)
            prefix.rename(install / 'prefix'); output.rename(install / 'locales')
            with patch.object(data, 'LOCK', lock_path), patch.object(data.platform, 'machine', return_value='x86_64'), \
                 patch.object(data.platform, 'libc_ver', return_value=('glibc', '2.39')), \
                 patch.object(data, 'authenticate', return_value={'verified': True}), \
                 patch.object(data, 'download', return_value=b'pinned'), \
                 patch.object(data, 'run', return_value=subprocess.CompletedProcess([], 0, b'fr_FR.UTF-8\n', b'')) as run:
                profile = data.provision(destination)
                self.assertEqual(profile['environment'], {'LOCPATH': str(install / 'locales')})
                self.assertTrue(profile['no_system_install']); self.assertFalse(profile['maintainer_scripts_run'])
                self.assertEqual(run.call_count, 1)
                (install / 'locales/LC_COLLATE').write_bytes(b'tampered')
                with self.assertRaisesRegex(data.ProfileError, 'compiled locale differs'): data.provision(destination)
                self.assertEqual(run.call_count, 1)

    def test_compiled_locpath_cannot_be_used_as_a_profile_root(self):
        with tempfile.TemporaryDirectory() as temporary:
            compiled = Path(temporary) / 'locales'
            french = compiled / 'fr_FR.UTF-8'
            french.mkdir(parents=True)
            (french / 'LC_COLLATE').write_bytes(b'compiled')
            before = data.tree_digest(compiled)
            with patch.object(data, 'authenticate') as authenticate:
                with self.assertRaisesRegex(data.ProfileError, 'compiled LOCPATH'):
                    data.provision(compiled)
                authenticate.assert_not_called()
            self.assertEqual(data.tree_digest(compiled), before)

    def test_unpinned_profile_and_wrong_platform_fail_before_authentication(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); lock = root / 'lock.json'; lock.write_text(json.dumps({'schema_version': 1, 'locale': 'fr_FR.UTF-8'}))
            with patch.object(data, 'LOCK', lock), patch.object(data.platform, 'machine', return_value='x86_64'), \
                 patch.object(data.platform, 'libc_ver', return_value=('glibc', '2.39')), patch.object(data, 'authenticate') as auth:
                with self.assertRaisesRegex(data.ProfileError, 'pinned content'): data.provision(root / 'profile')
                auth.assert_not_called()
            with patch.object(data.platform, 'machine', return_value='other'):
                with self.assertRaisesRegex(data.ProfileError, 'requires'): data.provision(root / 'other')
            with patch.object(data, 'provision', side_effect=data.ProfileError('bad')):
                self.assertEqual(data.main(['--root', str(root)]), 1)


if __name__ == '__main__':
    unittest.main()
