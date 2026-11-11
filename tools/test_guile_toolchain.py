# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import hashlib
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

import guile_frontend_gate as frontend
import guile_toolchain as g


class GuileToolchainTests(unittest.TestCase):
    def test_closure_is_exactly_pinned_with_authenticated_indices_and_licenses(self):
        lock = json.loads(g.LOCK.read_text())
        self.assertEqual(len(lock['packages']), 17)
        self.assertEqual(len({p['name'] for p in lock['packages']}), 17)
        self.assertEqual({index['path'] for index in lock['signed_indices']},
                         {'main/binary-amd64/Packages', 'universe/binary-amd64/Packages'})
        self.assertRegex(lock['installed_tree_sha256'], '^[0-9a-f]{64}$')
        for package in lock['packages']:
            self.assertTrue(package['url'].startswith('https://archive.ubuntu.com/ubuntu/pool/'))
            self.assertRegex(package['sha256'], '^[0-9a-f]{64}$')
            self.assertRegex(package['license_sha256'], '^[0-9a-f]{64}$')
            self.assertGreater(package['size'], 0)

    def test_changed_cached_bytes_and_symlinks_do_not_trigger_download_or_extraction(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(g.urllib.request, 'build_opener') as network:
            path = Path(tmp) / 'package.deb'; path.write_bytes(b'original')
            expected = hashlib.sha256(b'original').hexdigest()
            self.assertEqual(g.cached_download(path, 'https://example.com/pinned', expected, 8), b'original')
            path.write_bytes(b'changed!')
            with self.assertRaisesRegex(g.ProfileError, 'digest'): g.cached_download(path, 'https://example.com/pinned', expected, 8)
            path.unlink(); path.symlink_to(Path(tmp) / 'missing')
            with self.assertRaisesRegex(g.ProfileError, 'symlink'): g.cached_download(path, 'https://example.com/pinned', expected, 8)
            network.assert_not_called()

    def test_package_retries_response_read_and_caches_verified_bytes(self):
        payload = b'verified Guile package'
        stalled = MagicMock()
        stalled.__enter__.return_value = stalled
        stalled.read.side_effect = TimeoutError('stalled package response')
        with (tempfile.TemporaryDirectory() as tmp,
              patch.object(g.urllib.request, 'build_opener') as network,
              patch.object(g.oracle.time, 'sleep') as sleep):
            path = Path(tmp) / 'package.deb'
            transport = network.return_value.open
            transport.side_effect = [stalled, io.BytesIO(payload)]
            self.assertEqual(g.cached_download(path, 'https://example.com/package', g.digest(payload), len(payload)),
                             payload)
            self.assertEqual(path.read_bytes(), payload)
            self.assertEqual(list(Path(tmp).iterdir()), [path])
            self.assertEqual(transport.call_count, 2)
            sleep.assert_called_once_with(1)

    def test_release_retries_response_read_but_never_caches_wrong_digest(self):
        stalled = MagicMock()
        stalled.__enter__.return_value = stalled
        stalled.read.side_effect = TimeoutError('stalled release response')
        with (tempfile.TemporaryDirectory() as tmp,
              patch.object(g.urllib.request, 'build_opener') as network,
              patch.object(g.oracle.time, 'sleep') as sleep,
              patch.object(g, 'run') as verify):
            root = Path(tmp)
            lock = {'release_url': 'https://example.com/InRelease',
                    'release_sha256': g.digest(b'verified release')}
            transport = network.return_value.open
            transport.side_effect = [stalled, io.BytesIO(b'wrong release')]
            with self.assertRaisesRegex(g.ProfileError, 'pinned noble release differs'):
                g.authenticate(lock, root)
            self.assertFalse((root / 'provenance/noble-InRelease').exists())
            self.assertEqual(transport.call_count, 2)
            sleep.assert_called_once_with(1)
            verify.assert_not_called()

    def test_release_hash_section_rejects_duplicates_escape_and_malformed_records(self):
        identity = '1' * 64
        valid = f'SHA256:\n {identity} 12 main/binary-amd64/Packages\nOther: value\n'.encode()
        self.assertEqual(g.release_hashes(valid), {'main/binary-amd64/Packages': (identity, 12)})
        for invalid in (valid.replace(b'main/binary-amd64/Packages', b'../outside'),
                        valid.replace(b'Other: value', f' {identity} 12 main/binary-amd64/Packages'.encode()),
                        b'SHA256:\n invalid record\n', b'no hash section'):
            with self.assertRaises(g.ProfileError): g.release_hashes(invalid)

    def test_every_package_identity_field_is_checked_against_the_signed_index(self):
        record = json.loads(g.LOCK.read_text())['packages'][0]
        fields = {'Package':record['name'], 'Version':record['version'], 'Architecture':record['architecture'],
                  'SHA256':record['sha256'], 'Size':str(record['size']),
                  'Filename':record['url'].removeprefix('https://archive.ubuntu.com/ubuntu/')}
        g.validate_package(record, fields)
        for key in fields:
            with self.assertRaises(g.ProfileError): g.validate_package(record, {**fields, key:'changed'})

    def test_tree_identity_covers_bytes_modes_symlink_targets_and_hardlink_topology(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); (root / 'file').write_bytes(b'original')
            (root / 'link').symlink_to('file')
            os.link(root / 'file', root / 'hard')
            original = g.tree_digest(root)
            (root / 'file').write_bytes(b'changed')
            self.assertNotEqual(g.tree_digest(root), original)
            (root / 'file').write_bytes(b'original')
            (root / 'hard').unlink(); (root / 'hard').write_bytes(b'original')
            self.assertNotEqual(g.tree_digest(root), original)
            (root / 'hard').unlink(); os.link(root / 'file', root / 'hard')
            (root / 'link').unlink(); (root / 'link').symlink_to('hard')
            self.assertNotEqual(g.tree_digest(root), original)
            (root / 'link').unlink(); (root / 'link').symlink_to('file')
            (root / 'file').chmod(0o600)
            self.assertNotEqual(g.tree_digest(root), original)

    def test_symlinked_release_is_rejected_before_following_it(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(g.urllib.request, 'build_opener') as network:
            root = Path(tmp); (root / 'provenance').mkdir()
            outside = root / 'must-not-write'
            (root / 'provenance/noble-InRelease').symlink_to(outside)
            with self.assertRaisesRegex(g.ProfileError, 'symlink'):
                g.authenticate({}, root)
            self.assertFalse(outside.exists()); network.assert_not_called()

    def test_wrapper_quotes_profile_paths_and_pins_runtime_environment(self):
        with tempfile.TemporaryDirectory(prefix="rmake-guile path '$ value-") as tmp:
            root = Path(tmp); g.wrappers(root)
            wrapper = root / 'bin/guile'
            self.assertIn('GUILE_AUTO_COMPILE=0', wrapper.read_text())
            self.assertIn('PKG_CONFIG_SYSROOT_DIR=', (root / 'bin/pkg-config').read_text())
            g.run(['/bin/sh', '-n', str(wrapper)])
            self.assertEqual(g.environment(root)['RMAKE_GUILE_PREFIX'], str(g.prefix_for(root)))

    def test_component_runner_cannot_inherit_a_single_child_case_or_default_target(self):
        with patch.dict(os.environ, {'RMAKE_GUILE_COMPONENT_CASE':'outside'}), patch.object(g.subprocess, 'run') as execute:
            g.component_tests(Path('/reviewed/profile'))
            arguments = execute.call_args.kwargs
            self.assertNotIn('RMAKE_GUILE_COMPONENT_CASE', arguments['env'])
            self.assertEqual(arguments['env']['CARGO_TARGET_DIR'], str(g.ROOT / 'target/guile-component'))
            self.assertEqual(arguments['env']['GUILE_AUTO_COMPILE'], '0')
            self.assertIn('guile_dispatcher', execute.call_args.args[0])

    def test_verified_warm_reference_republishes_the_profile_consumed_by_the_gate(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            destination = root / 'sdk'
            references = destination / 'references'
            references.mkdir(parents=True)
            archive = root / '.tools/downloads/make-4.4.1.tar.gz'
            archive.parent.mkdir(parents=True)
            archive.write_bytes(b'locked GNU input')
            lock = root / 'oracle/guile/toolchain.json'
            lock.parent.mkdir(parents=True)
            lock.write_bytes(b'locked SDK input')
            source_identity = g.digest(archive.read_bytes())
            (root / 'oracle/manifest.json').write_text(json.dumps({'archive': {'sha256': source_identity}}))
            inputs = {
                'source_sha256': source_identity,
                'tool_lock_sha256': g.digest(lock.read_bytes()),
                'compiler_sha256': g.digest(Path('/usr/bin/cc').read_bytes()),
                'destination': str(destination),
            }
            binary = root / 'reference-make'
            binary.write_bytes(b'verified build')
            library = root / 'libguile.so'
            library.write_bytes(b'verified runtime')
            profile = {
                'inputs': inputs,
                'binary': {'path': str(binary), 'sha256': g.digest(binary.read_bytes())},
                'runtime_dependencies': [{'path': str(library), 'sha256': g.digest(library.read_bytes())}],
            }
            key = g.digest(json.dumps(inputs, sort_keys=True).encode())
            record = json.dumps(profile, indent=2).encode() + b'\n'
            (references / (key + '.json')).write_bytes(record)
            alias = destination / 'reference-profile.json'
            with patch.object(g, 'ROOT', root), patch.object(g, 'LOCK', lock), patch.object(g.subprocess, 'run') as execute:
                for old in (None, b'{"stale":true}\n'):
                    with self.subTest(previous=old):
                        alias.unlink(missing_ok=True)
                        if old is not None:
                            alias.write_bytes(old)
                        self.assertEqual(g.build_reference(destination), profile)
                        self.assertTrue(alias.is_file())
                        self.assertEqual(alias.read_bytes(), record)
                library.write_bytes(b'changed runtime')
                alias.write_bytes(b'must not republish')
                with self.assertRaisesRegex(g.ProfileError, 'cached enabled-reference runtime differs'):
                    g.build_reference(destination)
                self.assertEqual(alias.read_bytes(), b'must not republish')
                execute.assert_not_called()

    def test_frontend_gate_cannot_inherit_single_child_selectors(self):
        selectors = {
            'RMAKE_GUILE_COMPONENT_CASE': 'outside',
            'RMAKE_NATIVE_ENTRY_CASE': 'one-case',
            'RMAKE_STARTUP_OWNER_UNIT_CHILD': '1',
        }
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            output = root / 'reports'
            defaults = []
            for label in ('debug', 'release'):
                default = root / f'default-{label}'
                default.touch()
                defaults.append(default)
                enabled = output / 'cargo-target' / label / 'rmake'
                enabled.parent.mkdir(parents=True)
                enabled.touch()
            locale_root = root / 'separate-locale-cache'
            arguments = ['guile_frontend_gate.py', '--profile', str(root / 'reference-profile.json'),
                         '--default-debug', str(defaults[0]), '--default-release', str(defaults[1]),
                         '--output', str(output), '--locale-root', str(locale_root)]
            profile = {'environment': g.environment(root / 'sdk')}
            with (patch.object(sys, 'argv', arguments), patch.dict(os.environ, selectors),
                  patch.object(frontend, 'read_profile', return_value=profile),
                  patch.object(frontend.subprocess, 'run') as execute):
                self.assertEqual(frontend.main(), 0)
                commands = execute.call_args_list
                workspace_tests = [command.args[0] for command in commands
                                   if command.args[0][1:4] == ['tools/rust.py', 'cargo', 'test']]
                self.assertEqual(len(workspace_tests), 1)
                command, = workspace_tests
                self.assertIn('--all-targets', command)
                self.assertIn('--workspace', command)
                self.assertIn('--locked', command)
                feature = command.index('--features')
                self.assertEqual(command[feature + 1], 'guile')
                for script in ('guile_detached.py', 'guile_eventfd_pressure.py',
                               'guile_detached_parallel.py', 'guile_parallel_pressure.py'):
                    selected = [call.args[0] for call in commands
                                if call.args[0][1:2] == ['tools/' + script]]
                    with self.subTest(script=script):
                        self.assertEqual([
                            Path(argv[argv.index('--candidate') + 1]).name for argv in selected
                        ], ['enabled-debug', 'enabled-release'])
                comparisons = [call.args[0] for call in commands
                               if call.args[0][1:2] == ['tools/guile_frontend.py']]
                self.assertEqual(len(comparisons), 4)
                for argv in comparisons:
                    self.assertEqual(argv[argv.index('--locale-root') + 1], str(locale_root))
                for command in commands:
                    self.assertTrue(selectors.keys().isdisjoint(command.kwargs['env']), command.args)
                self.assertEqual({key: os.environ[key] for key in selectors}, selectors)


if __name__ == '__main__': unittest.main()
