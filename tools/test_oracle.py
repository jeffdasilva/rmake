# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original tests for provenance, archive safety and reference identity checks."""

import copy
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import time
import unittest
from unittest import mock

import oracle


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / "archive.tar.gz"

    def write_archive(self, entries):
        with tarfile.open(self.archive, "w:gz") as archive:
            for name, kind, value in entries:
                member = tarfile.TarInfo(name)
                member.mtime = 1234567890
                member.mode = 0o4755
                if kind == "file":
                    data = value.encode()
                    member.size = len(data)
                    archive.addfile(member, io.BytesIO(data))
                else:
                    member.type = kind
                    member.linkname = value
                    archive.addfile(member)

    def test_extracts_regular_files_without_set_id_and_preserves_mtime(self):
        self.write_archive([("make-4.4.1/configure", "file", "example")])
        extracted = oracle.safe_extract(self.archive, self.root / "source", "make-4.4.1")
        result = extracted / "configure"
        self.assertEqual(result.read_text(), "example")
        self.assertEqual(result.stat().st_mode & 0o7777, 0o755)
        self.assertEqual(result.stat().st_mtime, 1234567890)

    def test_rejects_traversal_absolute_wrong_root_and_backslashes(self):
        for name in ("../escape", "/escape", "other/file", "make-4.4.1/../../escape", "make-4.4.1/evil\\name"):
            with self.subTest(name=name):
                self.write_archive([("make-4.4.1/good", "file", "ok"), (name, "file", "bad")])
                with self.assertRaisesRegex(oracle.OracleError, "unsafe archive path"):
                    oracle.safe_extract(self.archive, self.root / "source", "make-4.4.1")
                self.assertEqual(list((self.root / "source").iterdir()), [])
                self.assertFalse((self.root / "escape").exists())

    def test_rejects_links_and_special_files_before_writing(self):
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.CHRTYPE, tarfile.FIFOTYPE):
            with self.subTest(kind=kind):
                self.write_archive([("make-4.4.1/link", kind, "/tmp")])
                with self.assertRaisesRegex(oracle.OracleError, "unsafe archive entry type"):
                    oracle.safe_extract(self.archive, self.root / "source", "make-4.4.1")

    def test_rejects_duplicate_normalized_paths(self):
        self.write_archive([("make-4.4.1/a", "file", "one"), ("make-4.4.1/./a", "file", "two")])
        with self.assertRaisesRegex(oracle.OracleError, "duplicate archive path"):
            oracle.safe_extract(self.archive, self.root / "source", "make-4.4.1")

    def test_rejects_preexisting_destination(self):
        self.write_archive([("make-4.4.1/a", "file", "one")])
        destination = self.root / "source"
        destination.mkdir()
        (destination / "valuable").write_text("keep")
        with self.assertRaisesRegex(oracle.OracleError, "not empty"):
            oracle.safe_extract(self.archive, destination, "make-4.4.1")
        self.assertEqual((destination / "valuable").read_text(), "keep")

    def test_rejects_extraction_bomb_before_writing(self):
        self.write_archive([("make-4.4.1/a", "file", "too large")])
        with mock.patch.object(oracle, "MAX_UNPACKED_BYTES", 1):
            with self.assertRaisesRegex(oracle.OracleError, "extraction limits"):
                oracle.safe_extract(self.archive, self.root / "source", "make-4.4.1")
        self.assertEqual(list((self.root / "source").iterdir()), [])

    def test_corrupt_cached_download_is_not_executed_or_silently_replaced(self):
        cache = self.root / "downloads"
        cache.mkdir()
        (cache / "make-4.4.1.tar.gz").write_bytes(b"corrupt")
        with mock.patch.object(oracle.urllib.request, "build_opener") as network:
            with self.assertRaisesRegex(oracle.OracleError, "archive SHA256 mismatch.*archive was not executed"):
                oracle.download_archive(oracle.load_config(), cache)
            network.assert_not_called()

    def test_refuses_redirect_to_plain_http(self):
        with self.assertRaisesRegex(oracle.OracleError, "away from HTTPS"):
            oracle.HTTPSRedirectsOnly().redirect_request(None, None, 302, "", {}, "http://example.invalid/archive")


class ProfileTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / "make"
        self.binary.write_bytes(b"original reference bytes")
        self.binary.chmod(0o755)
        self.profile_path = self.root / "profile.json"
        self.config = oracle.load_config()
        self.observed = {
            "version_output": "GNU Make 4.4.1",
            "variables": {"MAKE_VERSION": "4.4.1", ".FEATURES": "jobserver"},
            "runtime": {"architecture": "x86_64", "libc": ["glibc", "2.39"]},
        }
        self.version = mock.patch.object(oracle, "version_output", return_value="GNU Make 4.4.1")
        self.version_mock = self.version.start()
        self.addCleanup(self.version.stop)
        self.observation = mock.patch.object(oracle, "observe", return_value=copy.deepcopy(self.observed))
        self.observation_mock = self.observation.start()
        self.addCleanup(self.observation.stop)
        self.profile = copy.deepcopy(oracle.create_profile(self.binary, self.config, {"compiler": "test compiler"}))
        self.save()

    def save(self, reseal=False):
        if reseal:
            self.profile["profile_id"] = oracle.json_digest({k: v for k, v in self.profile.items() if k != "profile_id"})
        self.profile_path.write_text(json.dumps(self.profile))

    def verify(self):
        return oracle.verify(self.binary, self.profile_path, self.config)

    def test_verifies_recorded_binary_and_profile(self):
        self.assertEqual(self.verify()["profile_id"], self.profile["profile_id"])

    def test_rejects_changed_binary_even_if_it_reports_the_right_version(self):
        self.binary.write_bytes(b"modified reference bytes")
        self.version_mock.reset_mock()
        with self.assertRaisesRegex(oracle.OracleError, "binary SHA256 mismatch"):
            self.verify()
        self.version_mock.assert_not_called()

    def test_rejects_wrong_binary_version_after_digest_check(self):
        self.version_mock.return_value = "GNU Make 4.3"
        with self.assertRaisesRegex(oracle.OracleError, "oracle version mismatch"):
            self.verify()

    def test_rejects_tampered_manifest_version_even_if_resealed(self):
        self.profile["oracle_version"] = "4.3"
        self.save(reseal=True)
        with self.assertRaisesRegex(oracle.OracleError, "profile version mismatch"):
            self.verify()

    def test_rejects_tampered_feature_profile(self):
        self.profile["observed"]["variables"][".FEATURES"] = "guile"
        self.save()
        with self.assertRaisesRegex(oracle.OracleError, "profile integrity mismatch"):
            self.verify()

    def test_rejects_resealed_profile_observation_mismatch(self):
        self.profile["observed"]["variables"][".FEATURES"] = "guile"
        self.save(reseal=True)
        with self.assertRaisesRegex(oracle.OracleError, "profile drift in variables"):
            self.verify()

    def test_rejects_runtime_drift(self):
        self.observation_mock.return_value["runtime"]["libc"] = ["glibc", "9.99"]
        with self.assertRaisesRegex(oracle.OracleError, "profile drift in runtime"):
            self.verify()

    def test_rejects_source_config_drift(self):
        self.config["cflags"] = "-O0"
        with self.assertRaisesRegex(oracle.OracleError, "source/configuration mismatch"):
            self.verify()

    def test_rejects_resealed_archive_provenance_change(self):
        self.profile["source"] = copy.deepcopy(self.profile["source"])
        self.profile["source"]["sha256"] = "0" * 64
        self.save(reseal=True)
        with self.assertRaisesRegex(oracle.OracleError, "source/configuration mismatch"):
            self.verify()

    def test_rejects_malformed_and_missing_profile_fields(self):
        for malformed in ([], None, "unexpected"):
            with self.subTest(malformed=malformed):
                self.profile["observed"] = malformed
                self.save(reseal=True)
                with self.assertRaisesRegex(oracle.OracleError, "malformed oracle profile"):
                    self.verify()
        del self.profile["binary"]
        self.save(reseal=True)
        with self.assertRaisesRegex(oracle.OracleError, "malformed oracle profile"):
            self.verify()

    def test_rejects_invalid_json_profile(self):
        self.profile_path.write_text("{")
        with self.assertRaisesRegex(oracle.OracleError, "cannot read"):
            self.verify()


class ExecutionTests(unittest.TestCase):
    def test_accepts_only_exact_gnu_version(self):
        oracle.require_version("GNU Make 4.4.1\nCopyright", "4.4.1")
        for output in ("GNU Make 4.3", "GNU Make 4.4.10", "rmake 4.4.1", "", "GNU Make 4.4.1 patched"):
            with self.subTest(output=output):
                with self.assertRaisesRegex(oracle.OracleError, "version mismatch"):
                    oracle.require_version(output, "4.4.1")

    def test_clean_environment_does_not_inherit_make_or_loader_controls(self):
        with mock.patch.dict(oracle.os.environ, {"MAKEFLAGS": "-n", "MAKEFILES": "/evil", "LD_PRELOAD": "/evil"}):
            result = oracle.clean_environment()
        self.assertEqual(result["LC_ALL"], "C")
        for name in ("MAKEFLAGS", "MAKEFILES", "LD_PRELOAD", "CC", "BASH_ENV"):
            self.assertNotIn(name, result)

    def test_command_timeout_is_bounded(self):
        started = time.monotonic()
        with self.assertRaisesRegex(oracle.OracleError, "exceeded"):
            oracle.checked_run([sys.executable, "-c", "import time; time.sleep(30)"],
                               cwd=oracle.ROOT, env=oracle.clean_environment(), timeout=0.1)
        self.assertLess(time.monotonic() - started, 5)

    def test_missing_command_has_clear_error(self):
        with self.assertRaisesRegex(oracle.OracleError, "cannot run"):
            oracle.checked_run(["/does-not-exist/rmake-test"], cwd=oracle.ROOT, env=oracle.clean_environment())

    def test_cli_rejects_host_make_if_it_is_not_4_4_1(self):
        host = Path("/usr/bin/make")
        if not host.exists():
            self.skipTest("host make unavailable")
        version = subprocess.run([str(host), "--version"], capture_output=True, text=True, check=True).stdout
        if version.splitlines()[0] == "GNU Make 4.4.1":
            self.skipTest("host already is the pinned GNU version")
        result = subprocess.run([sys.executable, str(oracle.ROOT / "tools/oracle.py"), "verify", "--binary", str(host)],
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 2)
        # A foreign binary is rejected without running it; a fresh checkout
        # may reject earlier because no recorded oracle identity exists yet.
        self.assertTrue("SHA256 mismatch" in result.stderr or "cannot read" in result.stderr, result.stderr)
        self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
