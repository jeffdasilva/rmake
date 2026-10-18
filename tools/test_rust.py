# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import os
from unittest import mock
import unittest

import rust


class RustToolchainTests(unittest.TestCase):
    def test_local_environment_pins_toolchain_without_changing_process(self):
        with mock.patch.dict(os.environ, {"RUSTUP_TOOLCHAIN": "nightly"}):
            env = rust.local_environment()
            self.assertEqual(env["RUSTUP_TOOLCHAIN"], rust.TOOLCHAIN)
            self.assertEqual(os.environ["RUSTUP_TOOLCHAIN"], "nightly")

    def test_rejects_compiler_overrides_before_running_cargo(self):
        for name in ("RUSTC", "RUSTC_WRAPPER", "CARGO_BUILD_RUSTC"):
            with self.subTest(name=name), mock.patch.dict(os.environ, {name: "/wrong/compiler"}), \
                    mock.patch.object(rust.subprocess, "check_output") as check:
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
