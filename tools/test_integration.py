# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
import gzip
import hashlib
import io
import json
import socket
from pathlib import Path
import tarfile
import tempfile
import unittest
import urllib.error
from unittest.mock import patch

import conformance as h
import integration as i
import integration_records as records


def reply(payload, content_length=None):
    response = io.BytesIO(payload)
    response.headers = {} if content_length is None else {'Content-Length': str(content_length)}
    return response


class IntegrationTests(unittest.TestCase):
    def test_sources_and_build_plans_are_pinned_and_have_independent_smokes(self):
        data = json.loads(i.MANIFEST.read_text())
        self.assertEqual(data['schema_version'], 1)
        self.assertEqual({p['id'] for p in data['projects']},
                         {'xxhash-0.8.3', 'zlib-1.3.2', 'lua-5.4.9', 'gzip-1.14', 'patch-2.8',
                          'gnu-make-4.4.1', 'zstd-1.5.7', 'busybox-1.37.0'})
        make = next(p for p in data['projects'] if p['id'] == 'gnu-make-4.4.1')
        archive = i.oracle.load_config()['archive']
        self.assertEqual((make['source_url'], make['archive_sha256'], make['topdir']),
                         (archive['url'], archive['sha256'], archive['root']))
        for project in data['projects']:
            self.assertRegex(project['archive_sha256'], r'^[0-9a-f]{64}$')
            self.assertRegex(project['license_sha256'], r'^[0-9a-f]{64}$')
            self.assertTrue(project['source_url'].startswith('https://'))
            if 'configuration' in project:
                config = project['configuration']
                self.assertEqual(project['target'], 'defconfig')
                self.assertRegex(config['stdout_sha256'], r'^[0-9a-f]{64}$')
                self.assertEqual(set(config['output_sha256']), {'.config', '.kernelrelease'})
                for name, digest in config['output_sha256'].items():
                    i.safe_name(name)
                    self.assertRegex(digest, r'^[0-9a-f]{64}$')
            else:
                self.assertTrue(bytes.fromhex(project['smoke_stdout_hex']))
                for name in [project['header'], *project['watched']]: i.safe_name(name)

    def test_parallel_gzip_profile_reuses_only_the_reviewed_source(self):
        plan = json.loads(i.MANIFEST.read_text())
        cases = i.project_profiles(plan)
        gzip, source = cases['gzip-1.14']
        parallel, pinned = cases['gzip-1.14-parallel-fifo']
        self.assertIs(gzip, source)
        self.assertIs(pinned, source)
        self.assertEqual(parallel['archive_sha256'], source['archive_sha256'])
        self.assertEqual(parallel['header_rebuild'], source['header_rebuild'])
        self.assertEqual(parallel['args'][:3], [
            '-j2', '--jobserver-style=fifo', '--output-sync=recurse'])
        self.assertEqual(bytes.fromhex(parallel['expected_build_stdout_hex']),
                         b'Making all in lib\nMaking all in .\n')
        bad = dict(plan['profiles'][0], project='unknown')
        with self.assertRaisesRegex(h.HarnessError, 'pinned source'):
            i.project_profiles(dict(plan, profiles=[bad]))
        for name in ('../outside', 'gzip-1.14'):
            bad = dict(plan['profiles'][0], id=name)
            with self.assertRaisesRegex(h.HarnessError, 'profile identifier'):
                i.project_profiles(dict(plan, profiles=[bad]))

    def test_shared_zlib_profile_uses_the_pinned_archive_and_safe_install(self):
        plan = json.loads(i.MANIFEST.read_text())
        cases = i.project_profiles(plan)
        zlib, source = cases['zlib-1.3.2']
        shared, pinned = cases['zlib-1.3.2-staged-shared']
        self.assertIs(zlib, source)
        self.assertIs(pinned, source)
        self.assertEqual(zlib['configure'][-1], '--static')
        self.assertEqual(shared['configure'][-2:], ['--shared', '--prefix=/usr'])
        self.assertEqual(i.STAGED_INSTALL_ARGS, ('DESTDIR=stage', 'LDCONFIG=:', 'install'))
        self.assertEqual(shared['archive_sha256'], source['archive_sha256'])
        self.assertEqual(shared['license_sha256'], source['license_sha256'])
        self.assertIn('libz.so.1.3.2', shared['watched'])
        witness = i.MANIFEST.parent / i.safe_name(shared['staged_install']['witness'])
        self.assertTrue(witness.is_file())
        self.assertNotEqual(shared['smoke_stdout_hex'],
                            shared['header_rebuild']['fresh_smoke_stdout_hex'])

    def test_busybox_build_reuses_the_config_source_and_pins_the_reviewed_edits(self):
        cases = i.project_profiles(json.loads(i.MANIFEST.read_text()))
        config, source = cases['busybox-1.37.0']
        build, pinned = cases['busybox-1.37.0-executable']
        self.assertIs(config, source)
        self.assertIs(pinned, source)
        self.assertEqual(config['target'], 'defconfig')
        self.assertEqual(build['configuration']['args'][-1], 'allnoconfig')
        self.assertEqual(build['target'], 'busybox')
        self.assertEqual(build['smoke'], ['./busybox', 'printf', '%s\\n', 'rmake-busybox'])
        self.assertEqual(bytes.fromhex(build['smoke_stdout_hex']), b'rmake-busybox\n')
        self.assertEqual(set(build['configuration']['build']['replace_lines'].values()),
                         {'CONFIG_BUSYBOX=y', 'CONFIG_PRINTF=y', 'CONFIG_SHOW_USAGE=y'})
        self.assertEqual(i.build_snapshot_limits(config), {})
        self.assertEqual(i.build_snapshot_limits(build),
                         {'max_files': 8192, 'max_total_bytes': 48 * 1024 * 1024})
        for limits in ({'max_files': 8193}, {'max_total_bytes': True}, {'max_files': -1},
                       {'timeout': 180}, []):
            with self.subTest(limits=limits), self.assertRaisesRegex(h.HarnessError, 'snapshot limits'):
                i.build_snapshot_limits({'snapshot': limits})

    def test_configuration_edits_require_unique_lines_and_a_pinned_result(self):
        old = b'# CONFIG_APP is not set\nother=value\n'
        new = b'CONFIG_APP=y\nother=value\n'
        plan = {'path': '.config', 'replace_lines': {'# CONFIG_APP is not set': 'CONFIG_APP=y'},
                'sha256': hashlib.sha256(new).hexdigest()}
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            config = work / '.config'
            config.write_bytes(old)
            i.edit_configuration(work, plan)
            self.assertEqual(config.read_bytes(), new)
            for initial, expected in ((old * 2, 'exactly one'), (new, 'exactly one'),
                                      (old.replace(b'other=value', b'other=different'), 'reviewed build input')):
                config.write_bytes(initial)
                with self.assertRaisesRegex(h.HarnessError, expected):
                    i.edit_configuration(work, plan)
                self.assertEqual(config.read_bytes(), initial)
            outside = work / 'real'
            config.rename(outside)
            config.symlink_to(outside)
            with self.assertRaisesRegex(h.HarnessError, 'regular file'):
                i.edit_configuration(work, plan)

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

    def test_missing_primary_moves_to_fallback_and_retries_transient_dns(self):
        source = b'verified source'
        primary = 'https://example.invalid/primary'
        fallback = 'https://example.invalid/fallback'
        project = {'id': 'unit', 'archive_sha256': hashlib.sha256(source).hexdigest(),
                   'source_url': primary, 'fallback_url': fallback}
        with tempfile.TemporaryDirectory() as tmp, patch.object(i.urllib.request, 'urlopen') as network, \
                patch.object(i.oracle.time, 'sleep') as sleep:
            network.side_effect = [urllib.error.HTTPError(primary, 404, 'not found', {}, None),
                                   urllib.error.URLError(socket.gaierror(socket.EAI_AGAIN, 'temporary DNS')),
                                   reply(source)]
            cache = Path(tmp)
            result = i.archive_for(project, cache)
            self.assertEqual(result.read_bytes(), source)
            self.assertEqual(list(cache.iterdir()), [result])
            self.assertEqual([call.args[0] for call in network.call_args_list], [primary, fallback, fallback])
            sleep.assert_called_once_with(1)

    def test_truncated_response_retries_before_publishing_verified_bytes(self):
        source = b'verified source'
        primary = 'https://example.invalid/primary'
        project = {'id': 'unit', 'archive_sha256': hashlib.sha256(source).hexdigest(),
                   'source_url': primary}
        with tempfile.TemporaryDirectory() as tmp, patch.object(i.urllib.request, 'urlopen') as network, \
                patch.object(i.oracle.time, 'sleep') as sleep:
            network.side_effect = [reply(source[:-3], len(source)), reply(source, len(source))]
            cache = Path(tmp)
            result = i.archive_for(project, cache)
            self.assertEqual(result.read_bytes(), source)
            self.assertEqual(list(cache.iterdir()), [result])
            self.assertEqual([call.args[0] for call in network.call_args_list], [primary, primary])
            sleep.assert_called_once_with(1)

    def test_bad_integrity_does_not_try_the_fallback(self):
        project = {'id': 'unit', 'archive_sha256': hashlib.sha256(b'expected').hexdigest(),
                   'source_url': 'https://example.invalid/primary', 'fallback_url': 'https://example.invalid/fallback'}
        with tempfile.TemporaryDirectory() as tmp, patch.object(i.urllib.request, 'urlopen') as network:
            network.side_effect = [reply(b'incorrect', 9), reply(b'expected', 8)]
            with self.assertRaisesRegex(h.HarnessError, 'pinned size/digest'):
                i.archive_for(project, Path(tmp))
            network.assert_called_once()
            self.assertEqual(list(Path(tmp).iterdir()), [])

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
        projects = {p['id']: p for p in json.loads(i.MANIFEST.read_text())['projects']}
        for name in ('lua-5.4.9', 'gzip-1.14', 'patch-2.8'):
            with self.subTest(project=name):
                project = projects[name]
                change = project['header_rebuild']
                old = bytes.fromhex(change['before_hex'])
                new = bytes.fromhex(change['after_hex'])
                self.assertNotEqual(bytes.fromhex(change['fresh_smoke_stdout_hex']),
                                    bytes.fromhex(project['smoke_stdout_hex']))
                self.assertEqual(i.rebuilt_header(project, b'prefix\n' + old + b'suffix'),
                                 b'prefix\n' + new + b'suffix')
                for content in (b'wrong upstream header', old + old, new):
                    with self.assertRaisesRegex(h.HarnessError, 'exactly one changed source fragment'):
                        i.rebuilt_header(project, content)

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
