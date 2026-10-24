#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Repository-local pinned Rust installation and Cargo invocation."""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / ".tools"
TOOLCHAIN = "1.90.0"
RUSTUP_VERSION = "1.28.2"
RUSTUP_SHA256 = "20a06e644b0d9bd2fbdbfd52d42540bdde820ea7df86e92e533c073da0cdd43c"
RUSTUP_URL = (
    f"https://static.rust-lang.org/rustup/archive/{RUSTUP_VERSION}/"
    "x86_64-unknown-linux-gnu/rustup-init"
)


def local_environment():
    env = os.environ.copy()
    env["CARGO_HOME"] = str(TOOLS / "cargo")
    env["RUSTUP_HOME"] = str(TOOLS / "rustup")
    env["RUSTUP_TOOLCHAIN"] = TOOLCHAIN
    env["PATH"] = str(TOOLS / "cargo" / "bin") + os.pathsep + env.get("PATH", "")
    return env


def bootstrap():
    if sys.platform != "linux" or os.uname().machine != "x86_64":
        raise RuntimeError("local Rust bootstrap currently supports x86_64 Linux only")
    rustup = TOOLS / "cargo" / "bin" / "rustup"
    env = local_environment()
    if not rustup.exists():
        downloads = TOOLS / "downloads"
        downloads.mkdir(parents=True, exist_ok=True)
        installer = downloads / f"rustup-init-{RUSTUP_VERSION}"
        with urllib.request.urlopen(RUSTUP_URL, timeout=60) as response:
            data = response.read(32 * 1024 * 1024 + 1)
        if hashlib.sha256(data).hexdigest() != RUSTUP_SHA256:
            raise RuntimeError("rustup installer SHA256 mismatch; refusing to execute")
        installer.write_bytes(data)
        installer.chmod(0o700)
        subprocess.run(
            [str(installer), "-y", "--no-modify-path", "--profile", "minimal",
             "--default-toolchain", TOOLCHAIN, "--component", "rustfmt", "--component", "clippy"],
            env=env, check=True,
        )
    else:
        subprocess.run(
            [str(rustup), "toolchain", "install", TOOLCHAIN, "--profile", "minimal",
             "--component", "rustfmt", "--component", "clippy"],
            env=env, check=True,
        )


def cargo(args):
    # Recipes are marked recursive to receive pipe jobserver descriptors. Honor
    # make's non-executing modes instead of compiling during `make -n/-q/-t`.
    flags = os.environ.get("MAKEFLAGS", "").split()
    short = flags[0].lstrip("-") if flags and not flags[0].startswith("--") else ""
    if "=" not in short and any(flag in short for flag in "nqt"):
        return 0
    local_cargo = TOOLS / "cargo" / "bin" / "cargo"
    if local_cargo.exists():
        executable, env = str(local_cargo), local_environment()
    else:
        executable, env = shutil.which("cargo"), os.environ.copy()
    if not executable:
        raise RuntimeError("Rust is unavailable; run make bootstrap-rust")
    for name in ("RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                 "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER",
                 "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"):
        if env.get(name):
            raise RuntimeError(f"unset {name} to use the pinned compiler")
    version = subprocess.check_output([executable, "--version"], env=env, text=True)
    if version.split()[1] != TOOLCHAIN:
        raise RuntimeError(f"expected Cargo {TOOLCHAIN}, got {version.strip()}")
    compiler = shutil.which("rustc", path=env.get("PATH"))
    if not compiler:
        raise RuntimeError("rustc is unavailable; run make bootstrap-rust")
    rust_version = subprocess.check_output([compiler, "--version"], env=env, text=True)
    if rust_version.split()[1] != TOOLCHAIN:
        raise RuntimeError(f"expected rustc {TOOLCHAIN}, got {rust_version.strip()}")
    env["RUSTC"] = compiler
    # Match the repository's pinned GNU installation prefix. Ordinary cargo
    # builds default to /usr/local/include; packagers can configure this value.
    env.setdefault("RMAKE_INCLUDEDIR", str(TOOLS / "oracle" / "include"))
    # Exec preserves make's inherited pipe-jobserver descriptors. A subprocess
    # with Python's default close_fds would silently discard those descriptors.
    os.execvpe(executable, [executable, *args], env)


def main():
    try:
        if len(sys.argv) == 2 and sys.argv[1] == "bootstrap":
            bootstrap()
            return 0
        if len(sys.argv) >= 2 and sys.argv[1] == "cargo":
            return cargo(sys.argv[2:])
        print("usage: rust.py bootstrap | cargo [ARGS...]", file=sys.stderr)
        return 2
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"rust toolchain: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
