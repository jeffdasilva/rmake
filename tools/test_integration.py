# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import conformance as h
import integration as i
import integration_records as records


class IntegrationTests(unittest.TestCase):
    def test_sources_and_build_plans_are_pinned_and_have_independent_smokes(self):
        data = json.loads(i.MANIFEST.read_text())
        self.assertEqual(data['schema_version'], 1)
        self.assertEqual({p['id'] for p in data['projects']}, {'xxhash-0.8.3', 'zlib-1.3.2', 'lua-5.4.9'})
        for project in data['projects']:
            self.assertRegex(project['archive_sha256'], r'^[0-9a-f]{64}$')
            self.assertRegex(project['license_sha256'], r'^[0-9a-f]{64}$')
            self.assertTrue(project['source_url'].startswith('https://'))
            self.assertTrue(bytes.fromhex(project['smoke_stdout_hex']))
            for name in [project['header'], *project['watched']]: i.safe_name(name)

    def test_archive_cache_does_not_accept_changed_or_symlinked_sources(self):
        source = b'pinned bytes'
        project = {'id': 'unit', 'archive_sha256': hashlib.sha256(source).hexdigest()}
        with tempfile.TemporaryDirectory() as tmp, patch.object(i.urllib.request, 'urlopen') as network:
            root = Path(tmp)
            archive = root / 'unit.tar.gz'; archive.write_bytes(source)
            self.assertEqual(i.archive_for(project, root), archive)
            archive.write_bytes(b'changed')
            with self.assertRaisesRegex(h.HarnessError, 'digest'): i.archive_for(project, root)
            archive.unlink(); archive.symlink_to(root / 'missing')
            with self.assertRaisesRegex(h.HarnessError, 'symlink'): i.archive_for(project, root)
            network.assert_not_called()
        with self.assertRaisesRegex(h.HarnessError, 'identifier'):
            i.archive_for({'id': '../escape'}, Path('/not-written'))

    def test_archive_rejects_path_escape_special_files_and_escaping_links(self):
        for kind in ('path', 'special', 'link'):
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp); archive = root / 'input.tar.gz'
                with tarfile.open(archive, 'w:gz') as output:
                    item = tarfile.TarInfo('../outside' if kind == 'path' else 'unit/item')
                    if kind == 'special': item.type = tarfile.FIFOTYPE
                    if kind == 'link': item.type = tarfile.SYMTYPE; item.linkname = '../../outside'
                    output.addfile(item, io.BytesIO(b''))
                with self.assertRaises((h.HarnessError, tarfile.TarError)):
                    i.extract({'topdir': 'unit'}, archive, root / 'extract')
                self.assertFalse((root / 'outside').exists())

    def test_smoke_requires_exact_bytes_status_and_process_bounds(self):
        expected = b'independent\n'
        good = h.Observation(expected, b'', 0, None, {})
        self.assertTrue(i.validate_smoke(good, expected))
        from dataclasses import replace
        for bad in (replace(good, stdout=b'wrong'), replace(good, stderr=b'warning'),
                    replace(good, returncode=2), replace(good, limit='timeout'),
                    replace(good, leftover_processes=True)):
            self.assertFalse(i.validate_smoke(bad, expected))

    def test_noop_and_rebuild_freshness_are_independent_requirements(self):
        self.assertEqual(i.freshness_errors({'a': 10}, {'a': 10}, {'a': 11}), [])
        for noop, rebuilt in (({'a': 11}, {'a': 12}), ({'a': 10}, {'a': 10}), ({'a': 10}, {})):
            self.assertTrue(i.freshness_errors({'a': 10}, noop, rebuilt))

    def test_semantic_rebuild_requires_the_one_original_source_fragment(self):
        lua = next(p for p in json.loads(i.MANIFEST.read_text())['projects'] if p['id'] == 'lua-5.4.9')
        change = lua['header_rebuild']
        old = bytes.fromhex(change['before_hex'])
        new = bytes.fromhex(change['after_hex'])
        self.assertNotEqual(change['fresh_smoke_stdout_hex'], lua['smoke_stdout_hex'])
        self.assertEqual(i.rebuilt_header(lua, b'prefix\n' + old + b'suffix'), b'prefix\n' + new + b'suffix')
        for content in (b'wrong upstream header', old + old, new):
            with self.assertRaisesRegex(h.HarnessError, 'exactly one changed source fragment'):
                i.rebuilt_header(lua, content)

    def test_partial_raw_observation_is_preserved_with_checksums(self):
        with tempfile.TemporaryDirectory() as tmp:
            record = i.persist_part(Path(tmp), 'failure', {'stdout_hex': '00', 'status': 2})
            packed = Path(record['path']).read_bytes(); raw = records.restore(record)
            self.assertEqual(hashlib.sha256(packed).hexdigest(), record['sha256'])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), record['raw_sha256'])
            self.assertEqual(len(raw), record['raw_bytes'])
            self.assertEqual(json.loads(raw), {'stdout_hex': '00', 'status': 2})

    def test_lossless_shared_file_bytes_and_changed_blobs_are_detected(self):
        import base64
        data = b'\x00\xfforiginal bytes\n'
        value = {'filesystem': {'work/output': {'type': 'file', 'mode': 493,
                 'content': base64.b64encode(data).decode(), 'size': len(data),
                 'sha256': hashlib.sha256(data).hexdigest()}}}
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            first = i.persist_part(root, 'first', value)
            second = i.persist_part(root, 'second', value)
            self.assertEqual(len(list((root / 'blobs').iterdir())), 1)
            self.assertEqual(json.loads(records.restore(first)), value)
            self.assertEqual(records.restore(first), records.restore(second))
            blob = next((root / 'blobs').iterdir())
            blob.write_bytes(gzip.compress(b'changed', mtime=0))
            with self.assertRaisesRegex(h.HarnessError, 'blob digest'):
                records.restore(first)
            with self.assertRaisesRegex(h.HarnessError, 'blob changed'):
                i.persist_part(root, 'third', value)

    def test_archive_metadata_encoding_is_not_mistaken_for_a_file_blob(self):
        value = {'encoding': records.ENCODING, 'sha256': 'record digest', 'raw_bytes': 15}
        with tempfile.TemporaryDirectory() as tmp:
            record = i.persist_part(Path(tmp), 'metadata', value)
            self.assertEqual(json.loads(records.restore(record)), value)

    def test_unknown_project_never_downloads_or_runs_a_process(self):
        with patch.object(i.urllib.request, 'urlopen') as network, patch.object(h, 'capture') as execute:
            with self.assertRaises(SystemExit) as error: i.main(['--case', 'unreviewed'])
            self.assertEqual(error.exception.code, 2)
            network.assert_not_called(); execute.assert_not_called()


if __name__ == '__main__': unittest.main()
