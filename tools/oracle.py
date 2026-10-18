#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Build and verify the pinned GNU Make reference; Python standard library only.

The tracked manifest pins source provenance and the intended build. A local
profile records the resulting executable and runtime identities. Its digest is
an accidental-drift check, not a signature or an attestation against a hostile
local user. The archive is checked before any extracted code is executed.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "oracle" / "manifest.json"
PREFIX = ROOT / ".tools" / "oracle"
PROFILE = PREFIX / "profile.json"
SCHEMA_VERSION = 1
MAX_UNPACKED_BYTES = 128 * 1024 * 1024
MAX_ARCHIVE_MEMBERS = 10000
PROBE_VARIABLES = (
    "MAKE_VERSION", "MAKE_HOST", ".FEATURES", ".INCLUDE_DIRS", "SHELL",
    ".SHELLFLAGS", "AR", "ARFLAGS", "AS", "CC", "CXX", "CPP", "FC", "F77",
    "RM", "LINK.o", "COMPILE.c", "COMPILE.cc", "OUTPUT_OPTION",
)


class OracleError(Exception):
    """A safe, actionable oracle bootstrap/verification failure."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def json_digest(value: object) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def read_json(path: Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise OracleError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise OracleError(f"expected a JSON object in {path}")
    return value


def load_config(path: Path = CONFIG) -> dict:
    config = read_json(path)
    try:
        if config["schema_version"] != SCHEMA_VERSION:
            raise ValueError("unsupported schema_version")
        if config["version"] != "4.4.1":
            raise ValueError("this bootstrap supports only GNU Make 4.4.1")
        archive = config["archive"]
        if urllib.parse.urlsplit(archive["url"]).scheme != "https":
            raise ValueError("archive URL must use HTTPS")
        if not re.fullmatch(r"[a-f0-9]{64}", archive["sha256"]):
            raise ValueError("archive SHA256 must have 64 lowercase hexadecimal digits")
        if archive["root"] != f"make-{config['version']}":
            raise ValueError("unexpected archive root")
        if not 1 <= archive["max_download_bytes"] <= 64 * 1024 * 1024:
            raise ValueError("invalid download size limit")
        if "--without-guile" not in config["configure_args"]:
            raise ValueError("the baseline profile must disable Guile")
        for field in ("required_features", "forbidden_features", "configure_args"):
            if not isinstance(config[field], list) or not all(isinstance(v, str) for v in config[field]):
                raise ValueError(f"invalid {field}")
        if not isinstance(config["cflags"], str):
            raise ValueError("invalid cflags")
    except (KeyError, TypeError, ValueError) as error:
        raise OracleError(f"invalid oracle configuration {path}: {error}") from error
    return config


def clean_environment(home: Path | None = None) -> dict[str, str]:
    # In particular, never inherit MAKEFLAGS/MAKEFILES, CC/CFLAGS, LD_PRELOAD,
    # BASH_ENV, CONFIG_SITE, or user locale settings from the caller.
    return {
        "PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "TZ": "UTC",
        "HOME": str(home or Path("/nonexistent")), "CONFIG_SITE": "/dev/null",
    }


def checked_run(argv: list[str], *, cwd: Path, env: dict[str, str],
                timeout: float = 30, input_text: str | None = None,
                log=None) -> str:
    """Bound execution and kill the process group on timeout/interruption."""
    try:
        process = subprocess.Popen(
            argv, cwd=cwd, env=env, stdin=subprocess.PIPE if input_text is not None else subprocess.DEVNULL,
            stdout=log if log else subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, encoding="utf-8", errors="replace", start_new_session=True,
        )
    except OSError as error:
        raise OracleError(f"cannot run {argv[0]}: {error}") from error
    try:
        output, _ = process.communicate(input_text, timeout=timeout)
    except (subprocess.TimeoutExpired, KeyboardInterrupt) as error:
        with contextlib.suppress(ProcessLookupError):
            os.killpg(process.pid, signal.SIGKILL)
        process.communicate()
        if isinstance(error, KeyboardInterrupt):
            raise
        raise OracleError(f"command exceeded {timeout:g}s: {argv[0]}") from error
    if process.returncode:
        detail = f"\n{output[-4000:]}" if output else " (see build log)"
        raise OracleError(f"command failed ({process.returncode}): {' '.join(argv)}{detail}")
    return output or ""


class HTTPSRedirectsOnly(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if urllib.parse.urlsplit(newurl).scheme != "https":
            raise OracleError("refusing archive redirect away from HTTPS")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def verify_archive(path: Path, expected: str) -> None:
    actual = sha256_file(path)
    if actual != expected:
        raise OracleError(f"archive SHA256 mismatch for {path}: expected {expected}, got {actual}; archive was not executed")


def download_archive(config: dict, directory: Path) -> Path:
    archive = config["archive"]
    directory.mkdir(parents=True, exist_ok=True)
    destination = directory / f"make-{config['version']}.tar.gz"
    if destination.exists():
        verify_archive(destination, archive["sha256"])
        return destination
    opener = urllib.request.build_opener(HTTPSRedirectsOnly())
    request = urllib.request.Request(archive["url"], headers={"User-Agent": "rmake-oracle-bootstrap/1"})
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=directory, prefix=".download-", delete=False) as output:
            temporary = Path(output.name)
            started = time.monotonic()
            size = 0
            with opener.open(request, timeout=30) as response:
                if urllib.parse.urlsplit(response.url).scheme != "https":
                    raise OracleError("archive response did not use HTTPS")
                while chunk := response.read(64 * 1024):
                    size += len(chunk)
                    if size > archive["max_download_bytes"]:
                        raise OracleError("archive exceeds configured download size limit")
                    if time.monotonic() - started > 120:
                        raise OracleError("archive download exceeded 120s")
                    output.write(chunk)
        verify_archive(temporary, archive["sha256"])
        os.replace(temporary, destination)
    except (OSError, urllib.error.URLError) as error:
        raise OracleError(f"archive download failed: {error}") from error
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    return destination


def safe_extract(archive_path: Path, destination: Path, root_name: str) -> Path:
    """Extract only ordinary files/directories into a new empty directory.

    Reject links, special files, traversal, duplicate paths and tar bombs before
    writing anything. GNU Make's pinned release needs none of those entry types.
    Do not restore ownership, set-id bits, or arbitrary archive metadata.
    """
    destination.mkdir(parents=True, exist_ok=True)
    if any(destination.iterdir()):
        raise OracleError(f"archive destination is not empty: {destination}")
    try:
        with tarfile.open(archive_path, "r:gz") as archive:
            members = []
            seen = set()
            total = 0
            for member in archive:
                parts = PurePosixPath(member.name).parts
                if (not parts or parts[0] != root_name or member.name.startswith("/")
                        or ".." in parts or "\\" in member.name):
                    raise OracleError(f"unsafe archive path: {member.name!r}")
                normalized = PurePosixPath(*parts).as_posix()
                if normalized in seen:
                    raise OracleError(f"duplicate archive path: {member.name!r}")
                seen.add(normalized)
                if not (member.isdir() or member.isfile()) or member.size < 0:
                    raise OracleError(f"unsafe archive entry type: {member.name!r}")
                total += member.size
                members.append(member)
                if total > MAX_UNPACKED_BYTES or len(members) > MAX_ARCHIVE_MEMBERS:
                    raise OracleError("archive exceeds extraction limits")
            for member in members:
                target = destination.joinpath(*PurePosixPath(member.name).parts)
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                else:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with archive.extractfile(member) as source, target.open("xb") as output:
                        shutil.copyfileobj(source, output, length=1024 * 1024)
                    target.chmod(0o755 if member.mode & 0o111 else 0o644)
                    os.utime(target, (member.mtime, member.mtime))
    except (OSError, tarfile.TarError) as error:
        raise OracleError(f"cannot extract archive safely: {error}") from error
    return destination / root_name


def version_output(binary: Path) -> str:
    return checked_run([str(binary), "--version"], cwd=ROOT, env=clean_environment()).rstrip()


def require_version(output: str, expected: str) -> None:
    first = output.splitlines()[0] if output else ""
    if first != f"GNU Make {expected}":
        raise OracleError(f"oracle version mismatch: expected GNU Make {expected}, got {first!r}")


def probe_variables(binary: Path) -> dict[str, str]:
    # Original probe: no recipes run and no filesystem inputs are read. Preserve
    # feature ordering; stripping/sorting arbitrary output would hide drift.
    probe = "".join(f"$(info RMAKE_PROBE:{name}=$({name}))\n" for name in PROBE_VARIABLES)
    probe += ".PHONY: __rmake_oracle_probe__\n__rmake_oracle_probe__: ;\n"
    output = checked_run(
        [str(binary), "--no-print-directory", "-s", "-f", "-", "__rmake_oracle_probe__"],
        cwd=ROOT, env=clean_environment(), input_text=probe,
    )
    values = {}
    for line in output.splitlines():
        if not line.startswith("RMAKE_PROBE:") or "=" not in line:
            raise OracleError(f"unexpected oracle profile probe output: {line!r}")
        name, value = line[len("RMAKE_PROBE:"):].split("=", 1)
        if name in values:
            raise OracleError(f"duplicate oracle probe variable: {name}")
        values[name] = value
    if set(values) != set(PROBE_VARIABLES):
        raise OracleError("oracle profile probe omitted required variables")
    return values


def runtime_identity(binary: Path) -> dict:
    output = checked_run(["/usr/bin/ldd", str(binary)], cwd=ROOT, env=clean_environment())
    paths = []
    for line in output.splitlines():
        if "not found" in line:
            raise OracleError(f"oracle runtime dependency unavailable: {line.strip()}")
        match = re.search(r"(?:=>\s+|^\s*)(/\S+)", line)
        if match:
            paths.append(Path(match.group(1)))
    if not paths:
        raise OracleError("cannot identify dynamically linked oracle runtime")
    libraries = {str(path): {"resolved_path": str(path.resolve()), "sha256": sha256_file(path)}
                 for path in sorted(set(paths))}
    shell = Path("/bin/sh")
    os_release = Path("/etc/os-release")
    return {
        "system": platform.system(), "architecture": platform.machine(),
        "kernel": platform.release(), "libc": list(platform.libc_ver()),
        "os_release_sha256": sha256_file(os_release) if os_release.exists() else None,
        "shell": {"path": str(shell), "resolved_path": str(shell.resolve()), "sha256": sha256_file(shell)},
        "libraries": libraries,
        "observation_environment": clean_environment(),
    }


def observe(binary: Path, config: dict) -> dict:
    output = version_output(binary)
    require_version(output, config["version"])
    variables = probe_variables(binary)
    features = set(variables[".FEATURES"].split())
    missing = set(config["required_features"]) - features
    forbidden = set(config["forbidden_features"]) & features
    if missing or forbidden:
        raise OracleError(f"oracle feature profile mismatch: missing={sorted(missing)}, forbidden={sorted(forbidden)}")
    if variables["MAKE_VERSION"] != config["version"]:
        raise OracleError("MAKE_VERSION does not match the pinned oracle version")
    return {"version_output": output, "variables": variables, "runtime": runtime_identity(binary)}


def create_profile(binary: Path, config: dict, build: dict) -> dict:
    profile = {
        "schema_version": SCHEMA_VERSION,
        "oracle_version": config["version"],
        "configuration_sha256": json_digest(config),
        "source": config["archive"],
        "binary": {"relative_path": "bin/make", "sha256": sha256_file(binary)},
        "observed": observe(binary, config),
        "build": build,
        "scope": config["profile_scope"],
    }
    profile["profile_id"] = json_digest(profile)
    return profile


def verify(binary: Path = PREFIX / "bin" / "make", profile_path: Path = PROFILE,
           config: dict | None = None) -> dict:
    config = config if config is not None else load_config()
    binary = binary.absolute()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise OracleError(f"oracle executable missing or not executable: {binary}; run bootstrap")
    # Check the recorded digest before executing a potentially replaced file.
    # The binary itself is not a trusted source of its identity.
    profile = read_json(profile_path)
    try:
        if profile["schema_version"] != SCHEMA_VERSION:
            raise OracleError("oracle profile schema_version mismatch")
        if profile["oracle_version"] != config["version"]:
            raise OracleError("oracle profile version mismatch")
        if profile["configuration_sha256"] != json_digest(config) or profile["source"] != config["archive"]:
            raise OracleError("oracle profile source/configuration mismatch; explicit rebuild required")
        unsigned = {key: value for key, value in profile.items() if key != "profile_id"}
        if profile["profile_id"] != json_digest(unsigned):
            raise OracleError("oracle profile integrity mismatch")
        if not isinstance(profile["observed"], dict) or not isinstance(profile["binary"], dict):
            raise OracleError("malformed oracle profile observation/binary fields")
        actual = sha256_file(binary)
        if actual != profile["binary"]["sha256"]:
            raise OracleError(f"oracle binary SHA256 mismatch: expected {profile['binary']['sha256']}, got {actual}; executable was not run")
        require_version(version_output(binary), config["version"])
        observed = observe(binary, config)
        if observed != profile["observed"]:
            changed = [key for key in observed if observed[key] != profile["observed"].get(key)]
            raise OracleError(f"oracle profile drift in {', '.join(changed)}; explicit rebuild required")
    except (KeyError, TypeError) as error:
        raise OracleError(f"malformed oracle profile: missing/invalid {error}") from error
    return profile


def bootstrap(config: dict, *, jobs: int, rebuild: bool = False) -> dict:
    if platform.system() != "Linux":
        raise OracleError("the reference bootstrap supports Linux only")
    if not 1 <= jobs <= 16:
        raise OracleError("--jobs must be between 1 and 16")
    tools = ROOT / ".tools"
    tools.mkdir(exist_ok=True)
    # A nonblocking advisory lock prevents interleaved installs without making
    # an unrelated invocation wait indefinitely for a network/build process.
    with (tools / "oracle.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise OracleError("another oracle bootstrap is already running") from error
        if PREFIX.exists() and not rebuild:
            return verify(config=config)
        for command in ("cc", "make", "ldd"):
            if not shutil.which(command, path="/usr/bin:/bin"):
                raise OracleError(f"required build tool missing: {command}; install it before bootstrap")
        archive = download_archive(config, tools / "downloads")
        log_path = tools / "oracle-build.log"
        print(f"Building GNU Make {config['version']} with {jobs} jobs; log: {log_path}", file=sys.stderr)
        with tempfile.TemporaryDirectory(prefix=".oracle-build-", dir=tools) as temporary:
            work = Path(temporary)
            source = safe_extract(archive, work / "source", config["archive"]["root"])
            build_dir = work / "build"
            build_dir.mkdir()
            home = work / "home"
            home.mkdir()
            env = clean_environment(home)
            env["CFLAGS"] = config["cflags"]
            configure = [str(source / "configure"), f"--prefix={PREFIX}", *config["configure_args"]]
            compiler = checked_run(["cc", "--version"], cwd=build_dir, env=env).splitlines()[0]
            with log_path.open("w", encoding="utf-8") as log:
                for argv, timeout in (
                    (configure, 300),
                    (["make", f"-j{jobs}"], 600),
                    (["make", "install", f"DESTDIR={work / 'install'}"], 300),
                ):
                    log.write("$ " + " ".join(argv) + "\n")
                    log.flush()
                    checked_run(argv, cwd=build_dir, env=env, timeout=timeout, log=log)
            staged = work / "install" / PREFIX.relative_to("/")
            build_record = {
                "compiler": compiler, "compiler_sha256": sha256_file(Path(shutil.which("cc", path=env["PATH"]))),
                "configure_args": [f"--prefix={PREFIX}", *config["configure_args"]],
                "cflags": config["cflags"], "jobs": jobs,
                "config_log_sha256": sha256_file(build_dir / "config.log"),
                "build_log_sha256": sha256_file(log_path),
            }
            # Preserve release license and source provenance beside the local binary.
            shutil.copy2(source / "COPYING", staged / "COPYING")
            previous = work / "previous"
            if PREFIX.is_symlink():
                raise OracleError(f"refusing to replace symlinked oracle prefix: {PREFIX}")
            had_previous = PREFIX.exists()
            if had_previous:
                PREFIX.rename(previous)
            try:
                staged.rename(PREFIX)
                # Include-directory probing must happen after installation: GNU
                # omits built-in search directories that do not exist yet.
                profile = create_profile(PREFIX / "bin" / "make", config, build_record)
                PROFILE.write_text(json.dumps(profile, indent=2, sort_keys=True) + "\n", encoding="utf-8")
                return verify(config=config)
            except BaseException:
                # Roll back only the install we just created; preserve old identity.
                if PREFIX.exists():
                    shutil.rmtree(PREFIX)
                if had_previous:
                    previous.rename(PREFIX)
                raise


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("bootstrap", help="download, verify and build the pinned reference locally")
    build.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 1))
    build.add_argument("--rebuild", action="store_true", help="explicitly rebuild after source/runtime/profile drift")
    check = commands.add_parser("verify", help="reject wrong version, binary or local profile drift")
    check.add_argument("--binary", type=Path, default=PREFIX / "bin" / "make")
    check.add_argument("--profile", type=Path, default=PROFILE)
    for command in (build, check):
        command.add_argument("--json", action="store_true", help="print the verified profile as JSON")
    args = parser.parse_args(argv)
    try:
        config = load_config()
        if args.command == "bootstrap":
            profile = bootstrap(config, jobs=args.jobs, rebuild=args.rebuild)
            binary = PREFIX / "bin" / "make"
        else:
            binary = args.binary
            profile = verify(binary, args.profile, config)
        if args.json:
            print(json.dumps(profile, indent=2, sort_keys=True))
        else:
            print(f"Verified GNU Make {config['version']}: {binary}")
            print(f"Profile {profile['profile_id']}; binary SHA256 {profile['binary']['sha256']}")
        return 0
    except (OracleError, OSError) as error:
        print(f"oracle: {error}", file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        print("oracle: interrupted; no partial installation accepted", file=sys.stderr)
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
