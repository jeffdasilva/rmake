# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import hashlib
import io
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
from unittest import mock
import unittest

import rust


class RustToolchainTests(unittest.TestCase):
    def test_local_environment_pins_toolchain_without_changing_process(self):
        with mock.patch.dict(os.environ, {"RUSTUP_TOOLCHAIN": "nightly"}):
            env = rust.local_environment(rust.TOOLS)
            self.assertEqual(env["RUSTUP_TOOLCHAIN"], rust.TOOLCHAIN)
            self.assertEqual(os.environ["RUSTUP_TOOLCHAIN"], "nightly")

    def test_bootstrap_uses_the_installation_for_each_host(self):
        for machine, directory in (
            ("x86_64", ""),
            ("aarch64", "aarch64-unknown-linux-gnu"),
        ):
            with self.subTest(machine=machine), tempfile.TemporaryDirectory() as temporary:
                tools = Path(temporary)
                tool_root = tools / directory
                if machine == "aarch64":
                    legacy = tools / "cargo" / "bin" / "rustup"
                    legacy.parent.mkdir(parents=True)
                    legacy.write_bytes(b"x86_64 proxy")
                host = f"{machine}-unknown-linux-gnu"
                data = machine.encode()
                checksum = hashlib.sha256(data).hexdigest()
                with (mock.patch.object(rust, "TOOLS", tools),
                      mock.patch.object(rust.sys, "platform", "linux"),
                      mock.patch.object(rust.os, "uname", return_value=SimpleNamespace(machine=machine)),
                      mock.patch.dict(rust.RUSTUP_SHA256, {host: checksum}),
                      mock.patch.object(rust.urllib.request, "urlopen", return_value=io.BytesIO(data)) as download,
                      mock.patch.object(rust.subprocess, "run") as run):
                    rust.bootstrap()
                download.assert_called_once_with(
                    f"https://static.rust-lang.org/rustup/archive/1.28.2/{host}/rustup-init",
                    timeout=60,
                )
                installer = tool_root / "downloads" / "rustup-init-1.28.2"
                self.assertEqual(installer.read_bytes(), data)
                self.assertEqual(installer.stat().st_mode & 0o777, 0o700)
                self.assertEqual(run.call_args.args[0][0], str(installer))
                self.assertEqual(run.call_args.kwargs["env"]["CARGO_HOME"], str(tool_root / "cargo"))
                self.assertEqual(run.call_args.kwargs["env"]["RUSTUP_HOME"], str(tool_root / "rustup"))
                if machine == "aarch64":
                    self.assertEqual(legacy.read_bytes(), b"x86_64 proxy")
                    self.assertFalse((tools / "downloads").exists())

    def test_existing_arm_rustup_repairs_only_its_installation(self):
        host = "aarch64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as temporary:
            tools = Path(temporary)
            rustup = tools / host / "cargo" / "bin" / "rustup"
            rustup.parent.mkdir(parents=True)
            rustup.touch()
            with (mock.patch.object(rust, "TOOLS", tools),
                  mock.patch.object(rust, "rustup_host", return_value=host),
                  mock.patch.object(rust.urllib.request, "urlopen") as download,
                  mock.patch.object(rust.subprocess, "run") as run):
                rust.bootstrap()
            download.assert_not_called()
            self.assertEqual(run.call_args.args[0][:4], [str(rustup), "toolchain", "install", rust.TOOLCHAIN])
            self.assertEqual(run.call_args.kwargs["env"]["RUSTUP_HOME"], str(tools / host / "rustup"))

    def test_arm_checksum_mismatch_does_not_write_or_execute(self):
        host = "aarch64-unknown-linux-gnu"
        with tempfile.TemporaryDirectory() as temporary:
            tools = Path(temporary)
            with (mock.patch.object(rust, "TOOLS", tools),
                  mock.patch.object(rust, "rustup_host", return_value=host),
                  mock.patch.object(rust.urllib.request, "urlopen", return_value=io.BytesIO(b"wrong binary")),
                  mock.patch.object(rust.subprocess, "run") as run):
                with self.assertRaisesRegex(RuntimeError, "SHA256 mismatch"):
                    rust.bootstrap()
            run.assert_not_called()
            self.assertEqual(list((tools / host / "downloads").iterdir()), [])

    def test_unsupported_host_never_attempts_bootstrap(self):
        for platform, machine in (("darwin", "aarch64"), ("linux", "riscv64")):
            with (self.subTest(platform=platform, machine=machine),
                  mock.patch.object(rust.sys, "platform", platform),
                  mock.patch.object(rust.os, "uname", return_value=SimpleNamespace(machine=machine)),
                  mock.patch.object(rust.urllib.request, "urlopen") as download,
                  mock.patch.object(rust.subprocess, "run") as run):
                with self.assertRaisesRegex(RuntimeError, "x86_64 and AArch64 Linux"):
                    rust.bootstrap()
                download.assert_not_called()
                run.assert_not_called()

    def test_arm_cargo_selects_native_tool_root_or_system_fallback(self):
        host = "aarch64-unknown-linux-gnu"
        for installed in (False, True):
            with self.subTest(installed=installed), tempfile.TemporaryDirectory() as temporary:
                tools = Path(temporary)
                legacy_cargo = tools / "cargo" / "bin" / "cargo"
                legacy_cargo.parent.mkdir(parents=True)
                legacy_cargo.touch()
                tool_root = tools / host
                arm_cargo = tool_root / "cargo" / "bin" / "cargo"
                if installed:
                    arm_cargo.parent.mkdir(parents=True)
                    arm_cargo.touch()
                expected = str(arm_cargo) if installed else "/system/bin/cargo"
                compiler = str(arm_cargo.with_name("rustc")) if installed else "/system/bin/rustc"
                with (mock.patch.object(rust, "TOOLS", tools),
                      mock.patch.object(rust, "rustup_host", return_value=host),
                      mock.patch.dict(os.environ, {"PATH": "/system/bin"}, clear=True),
                      mock.patch.object(rust.shutil, "which",
                                        side_effect=lambda name, **_: expected if name == "cargo" else compiler),
                      mock.patch.object(rust.subprocess, "check_output",
                                        side_effect=[f"cargo {rust.TOOLCHAIN}", f"rustc {rust.TOOLCHAIN}"]) as check,
                      mock.patch.object(rust.os, "execvpe") as execute):
                    rust.cargo(["build"])
                self.assertEqual(check.call_args_list[0].args[0], [expected, "--version"])
                self.assertEqual(check.call_args_list[1].args[0], [compiler, "--version"])
                executable, argv, env = execute.call_args.args
                self.assertEqual(executable, expected)
                self.assertEqual(argv, [expected, "build"])
                self.assertEqual(env["RUSTC"], compiler)
                if installed:
                    self.assertEqual(env["CARGO_HOME"], str(tool_root / "cargo"))
                    self.assertEqual(env["RUSTUP_HOME"], str(tool_root / "rustup"))
                    self.assertEqual(env["PATH"].split(os.pathsep)[0], str(arm_cargo.parent))
                else:
                    self.assertNotIn("CARGO_HOME", env)
                    self.assertNotIn("RUSTUP_HOME", env)
                    self.assertEqual(env["PATH"], "/system/bin")

    def test_rejects_compiler_overrides_before_running_cargo(self):
        for name in ("RUSTC", "RUSTC_WRAPPER", "CARGO_BUILD_RUSTC"):
            with (self.subTest(name=name),
                  mock.patch.dict(os.environ, {name: "/wrong/compiler"}, clear=True),
                  mock.patch.object(rust.shutil, "which", return_value="/system/bin/cargo"),
                  mock.patch.object(rust.subprocess, "check_output") as check):
                with self.assertRaisesRegex(RuntimeError, "unset"):
                    rust.cargo(["build"])
                check.assert_not_called()

    def test_recursive_recipe_preserves_make_nonexecuting_modes(self):
        for flags in ("n", "-sn", "q", "t", "n --jobserver-auth=3,4"):
            with self.subTest(flags=flags), mock.patch.dict(os.environ, {"MAKEFLAGS": flags}), \
                    mock.patch.object(rust.subprocess, "check_output") as check:
                self.assertEqual(rust.cargo(["build"]), 0)
                check.assert_not_called()


if __name__ == "__main__":
    unittest.main()
