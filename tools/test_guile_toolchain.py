# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

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


if __name__ == '__main__': unittest.main()
