# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import developer_flows
import rust


X86 = "x86_64-unknown-linux-gnu"
ARM = "aarch64-unknown-linux-gnu"


class DeveloperFlowToolchainTests(unittest.TestCase):
    def setUp(self):
        temporary = self.enterContext(tempfile.TemporaryDirectory())
        self.root = Path(temporary)
        checkout = self.root / "checkout"
        checkout.mkdir()
        for name in ("Makefile", "tools", "oracle"):
            (checkout / name).symlink_to(developer_flows.ROOT / name)
        self.tools = checkout / ".tools"
        self.binary = self.root / "reference"
        self.binary.write_bytes(b"reference input")
        self.profile = self.root / "profile.json"
        self.profile.write_text("{}\n")
        self.enterContext(mock.patch.object(developer_flows, "ROOT", checkout))
        self.enterContext(mock.patch.object(rust, "TOOLS", self.tools))
        self.enterContext(mock.patch.object(developer_flows.oracle, "PROFILE", self.profile))

    def installed_tools(self, host):
        destination = self.tools if host == X86 else self.tools / host
        directory = destination / "rustup" / "toolchains" / f"{rust.TOOLCHAIN}-{host}" / "bin"
        directory.mkdir(parents=True)
        for name in ("cargo", "rustc"):
            executable = directory / name
            executable.write_text(f"{name} input for {host}\n")
            executable.chmod(0o755)
        return directory

    def subject(self, host):
        work = self.root / host
        work.mkdir()
        profile = {"profile_id": "test-profile", "binary": {"sha256": "test-digest"}}
        with mock.patch.object(rust, "rustup_host", return_value=host):
            return developer_flows.Subject(work, self.binary, profile, self.binary)

    def test_fixture_cargo_owner_matches_native_bootstrap_layout(self):
        installed = {host: self.installed_tools(host) for host in (X86, ARM)}
        for host in (X86, ARM):
            with self.subTest(host=host):
                subject = self.subject(host)
                tools = subject.work / ".tools"
                expected_root = tools if host == X86 else tools / ARM
                cargo_bin = expected_root / "cargo" / "bin"
                for name in ("cargo", "rustc"):
                    executable = cargo_bin / name
                    self.assertTrue(executable.is_symlink())
                    self.assertEqual(executable.resolve(), installed[host] / name)
                if host == ARM:
                    self.assertFalse((tools / "cargo").exists())
                with (mock.patch.object(rust, "TOOLS", tools),
                      mock.patch.object(rust, "rustup_host", return_value=host),
                      mock.patch.dict(os.environ, subject.environment, clear=True),
                      mock.patch.object(rust.subprocess, "check_output",
                                        side_effect=[f"cargo {rust.TOOLCHAIN}", f"rustc {rust.TOOLCHAIN}"]) as check,
                      mock.patch.object(rust.os, "execvpe") as execute):
                    rust.cargo(["--version"])
                self.assertEqual(
                    [call.args[0] for call in check.call_args_list],
                    [[str(cargo_bin / name), "--version"] for name in ("cargo", "rustc")],
                )
                executable, argv, environment = execute.call_args.args
                self.assertEqual(executable, str(cargo_bin / "cargo"))
                self.assertEqual(argv, [executable, "--version"])
                self.assertEqual(environment["CARGO_HOME"], str(expected_root / "cargo"))
                self.assertEqual(environment["RUSTUP_HOME"], str(expected_root / "rustup"))
                self.assertEqual(environment["RUSTC"], str(cargo_bin / "rustc"))

                protected = subject.protected()
                digest = developer_flows.h.sha256_file
                for name in ("Cargo.toml", "Cargo.lock"):
                    self.assertEqual(protected["files"][name], digest(developer_flows.FIXTURE / name))
                self.assertEqual(protected["source"]["tree/main.rs"]["sha256"],
                                 digest(developer_flows.FIXTURE / "src/main.rs"))

    def test_arm_fixture_requires_its_own_installed_compiler(self):
        legacy = self.installed_tools(X86)
        with self.assertRaisesRegex(developer_flows.h.HarnessError, "run make bootstrap-rust first"):
            self.subject(ARM)
        self.assertEqual((legacy / "cargo").read_text(), f"cargo input for {X86}\n")
        self.assertEqual((legacy / "rustc").read_text(), f"rustc input for {X86}\n")


if __name__ == "__main__":
    unittest.main()
