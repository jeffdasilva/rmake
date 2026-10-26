#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Independent, trusted-fixture GNU Make differential runner (M0).

This is process-group containment, NOT a filesystem/network security sandbox.
Run only reviewed fixtures. Comparisons do not yet cover timestamps, arbitrary
process events, PTYs, or stateful invocations. Never use this runner for untrusted
Makefiles. GNU Make is an external oracle, not a runtime implementation backend.
"""
from __future__ import annotations

import argparse
import base64
import dataclasses
import hashlib
import json
import math
import os
from pathlib import Path
import re
import resource
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
import tomllib
from typing import Any

import oracle as reference_oracle

ROOT = Path(__file__).resolve().parent.parent
MAX_OUTPUT = 1024 * 1024
MAX_FILES = 4096
MAX_FILE_BYTES = 16 * 1024 * 1024
MAX_CHILDREN = 128
CASE_KEYS = {"id", "requirement", "candidate", "args", "timeout_seconds",
             "environment", "expected", "output", "description", "provenance"}
ENV_PROTECTED = {"PATH", "HOME", "TMPDIR", "PWD", "OLDPWD", "LC_ALL", "LANG", "TZ"}


class HarnessError(Exception):
    """Invalid input, unavailable oracle, or bounded-observation failure."""


@dataclasses.dataclass(frozen=True)
class Case:
    path: Path
    id: str
    requirement: str
    candidate: str
    args: tuple[str, ...]
    timeout: float
    environment: dict[str, str]
    expected: dict[str, Any]
    output: str = "separate"


@dataclasses.dataclass
class Observation:
    stdout: bytes
    stderr: bytes
    returncode: int
    limit: str | None
    filesystem: dict[str, Any]
    leftover_processes: bool = False


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_fixture(root: Path) -> None:
    """Reject escaping links and special files before copying trusted inputs."""
    if root.is_symlink() or not root.is_dir():
        raise HarnessError(f"fixture must be a real directory: {root}")
    root = root.resolve()
    count, size = 0, 0
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in dirs + files:
            path = Path(directory) / name
            count += 1
            if count > MAX_FILES:
                raise HarnessError(f"fixture has too many entries: {root}")
            mode = path.lstat().st_mode
            if stat.S_ISLNK(mode):
                target = os.readlink(path)
                if os.path.isabs(target):
                    raise HarnessError(f"absolute fixture symlink: {path}")
                try:
                    path.resolve().relative_to(root)
                except (ValueError, RuntimeError, OSError) as error:
                    raise HarnessError(f"escaping or cyclic fixture symlink: {path}") from error
            elif stat.S_ISREG(mode):
                size += path.stat().st_size
                if size > MAX_FILE_BYTES:
                    raise HarnessError(f"fixture data exceeds byte limit: {root}")
            elif not stat.S_ISDIR(mode):
                raise HarnessError(f"special file in fixture: {path}")


def load_case(path: Path) -> Case:
    validate_fixture(path)
    manifest = path / "case.toml"
    if manifest.is_symlink() or not manifest.is_file():
        raise HarnessError(f"missing or symlinked case.toml: {path}")
    try:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except (ValueError, OSError) as error:
        raise HarnessError(f"invalid manifest {manifest}: {error}") from error
    if set(data) - CASE_KEYS:
        raise HarnessError(f"unknown fields in {manifest}: {sorted(set(data) - CASE_KEYS)}")
    case_id = data.get("id")
    if not isinstance(case_id, str) or not re.fullmatch(r"[a-z0-9][a-z0-9_-]*", case_id):
        raise HarnessError(f"invalid case id in {manifest}")
    if case_id != path.name:
        raise HarnessError(f"case id must match directory name: {manifest}")
    requirement = data.get("requirement")
    if not isinstance(requirement, str) or not requirement.strip():
        raise HarnessError(f"missing requirement in {manifest}")
    candidate = data.get("candidate", "pending")
    if candidate not in ("pending", "required"):
        raise HarnessError(f"candidate must be pending or required: {manifest}")
    args = data.get("args", ["-rR", "--no-print-directory"])
    if not isinstance(args, list) or any(not isinstance(x, str) or "\0" in x for x in args):
        raise HarnessError(f"args must be strings without NUL: {manifest}")
    timeout = data.get("timeout_seconds", 5)
    if (isinstance(timeout, bool) or not isinstance(timeout, (int, float))
            or not math.isfinite(timeout) or not 0 < timeout <= 60):
        raise HarnessError(f"timeout_seconds must be in (0, 60]: {manifest}")
    environment = data.get("environment", {})
    if not isinstance(environment, dict):
        raise HarnessError(f"environment must be a table: {manifest}")
    for key, value in environment.items():
        if (not re.fullmatch(r"[A-Za-z_.][A-Za-z_0-9.]*", key) or key in ENV_PROTECTED
                or not isinstance(value, str) or "\0" in value):
            raise HarnessError(f"invalid or reserved environment entry {key}: {manifest}")
    expected = data.get("expected", {})
    if not isinstance(expected, dict) or set(expected) - {
            "exit_code", "stdout", "stderr", "stdout_hex", "stderr_hex"}:
        raise HarnessError(f"invalid expected table: {manifest}")
    for key, value in expected.items():
        if key == "exit_code":
            if type(value) is not int or not -127 <= value <= 255:
                raise HarnessError(f"invalid expected exit_code: {manifest}")
        elif not isinstance(value, str):
            raise HarnessError(f"expected {key} must be a string: {manifest}")
        elif key.endswith("_hex") and not re.fullmatch(r"(?:[0-9A-Fa-f]{2})*", value):
            raise HarnessError(f"expected {key} must contain even-length ASCII hexadecimal: {manifest}")
    for stream in ("stdout", "stderr"):
        if stream in expected and stream + "_hex" in expected:
            raise HarnessError(f"expected {stream} and {stream}_hex are mutually exclusive: {manifest}")
    output = data.get("output", "separate")
    if output not in ("separate", "merged"):
        raise HarnessError(f"output must be separate or merged: {manifest}")
    for key in ("description", "provenance"):
        if key in data and not isinstance(data[key], str):
            raise HarnessError(f"{key} must be a string: {manifest}")
    return Case(path.resolve(), case_id, requirement, candidate, tuple(args),
                float(timeout), environment, expected, output)


def clean_environment(root: Path, additions: dict[str, str] | None = None) -> dict[str, str]:
    env = {"PATH": f"{root / 'bin'}:/usr/bin:/bin", "HOME": str(root / "home"),
           "TMPDIR": str(root / "tmp"), "LC_ALL": "C", "LANG": "C", "TZ": "UTC"}
    env.update(additions or {})
    return env


def snapshot(roots: dict[str, Path]) -> dict[str, Any]:
    """Capture bounded bytes/type/mode/link topology, never following symlinks.

    Timestamps are deliberately omitted in M0. Tests that require timestamp
    comparison must wait for an explicit relational comparator, not normalize
    wall-clock values and claim timestamp conformance.
    """
    result: dict[str, Any] = {}
    links: dict[tuple[int, int], list[str]] = {}
    total_bytes = 0

    def visit(path: Path, key: str) -> None:
        nonlocal total_bytes
        if len(result) >= MAX_FILES:
            raise HarnessError("filesystem observation exceeded entry limit")
        metadata = path.lstat()
        mode = metadata.st_mode
        item: dict[str, Any] = {"mode": stat.S_IMODE(mode)}
        result[key] = item
        if stat.S_ISLNK(mode):
            item.update(type="symlink", target=os.readlink(path))
        elif stat.S_ISDIR(mode):
            item["type"] = "directory"
            for child in sorted(path.iterdir(), key=lambda p: os.fsencode(p.name)):
                visit(child, f"{key}/{child.name}")
        elif stat.S_ISREG(mode):
            if metadata.st_size + total_bytes > MAX_FILE_BYTES:
                raise HarnessError("filesystem observation exceeded byte limit")
            descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
            try:
                opened = os.fstat(descriptor)
                if not stat.S_ISREG(opened.st_mode):
                    raise HarnessError("file changed type during observation")
                with os.fdopen(descriptor, "rb", closefd=False) as stream:
                    content = stream.read(MAX_FILE_BYTES - total_bytes + 1)
            finally:
                os.close(descriptor)
            total_bytes += len(content)
            if total_bytes > MAX_FILE_BYTES:
                raise HarnessError("filesystem observation exceeded byte limit")
            item.update(type="file", content=base64.b64encode(content).decode("ascii"),
                        size=len(content), sha256=hashlib.sha256(content).hexdigest())
            links.setdefault((metadata.st_dev, metadata.st_ino), []).append(key)
        elif stat.S_ISFIFO(mode):
            item["type"] = "fifo"
        elif stat.S_ISSOCK(mode):
            item["type"] = "socket"
        else:
            raise HarnessError(f"unsupported special output file: {key}")

    for label, root in sorted(roots.items()):
        if root.exists() or root.is_symlink():
            visit(root, label)
        else:
            result[label] = {"type": "missing"}
    for group in links.values():
        if len(group) > 1:
            for key in group:
                result[key]["hardlinks"] = sorted(group)
    return result


def remove_tree(path: Path) -> None:
    if path.is_symlink() or (path.exists() and not path.is_dir()):
        path.unlink()
    elif path.exists():
        # Fixtures may create read-only directories. Only reset our owned tree.
        def writable(function: Any, name: str, _error: Any) -> None:
            os.chmod(name, 0o700, follow_symlinks=False)
            function(name)
        shutil.rmtree(path, onerror=writable)


def reset_workspace(case: Case, root: Path, executable: Path) -> None:
    for name in ("work", "home", "tmp"):
        remove_tree(root / name)
        (root / name).mkdir(mode=0o755)
    # Preserve input hard links as well as mtimes without linking to source data.
    copied: dict[tuple[int, int], Path] = {}

    def copy_input(source: str | Path, destination: str | Path) -> str:
        metadata = os.stat(source, follow_symlinks=False)
        key = (metadata.st_dev, metadata.st_ino)
        if key in copied:
            os.link(copied[key], destination)
        else:
            shutil.copy2(source, destination)
            copied[key] = Path(destination)
        return str(destination)

    for source in case.path.iterdir():
        if source.name == "case.toml":
            continue
        destination = root / "work" / source.name
        if source.is_symlink():
            destination.symlink_to(os.readlink(source))
        elif source.is_dir():
            shutil.copytree(source, destination, symlinks=True, copy_function=copy_input)
        else:
            copy_input(source, destination)
    (root / "bin").mkdir(exist_ok=True)
    alias = root / "bin" / "make"
    alias.unlink(missing_ok=True)
    alias.symlink_to(executable)


def process_group_exists(group: int) -> bool:
    try:
        os.killpg(group, 0)
        return True
    except ProcessLookupError:
        return False


def kill_group(group: int) -> None:
    try:
        os.killpg(group, signal.SIGKILL)
    except ProcessLookupError:
        pass


def group_size(group: int) -> int:
    count = 0
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit():
            try:
                # /proc stat's comm can contain spaces and closing parentheses.
                fields = (entry / "stat").read_text().rsplit(")", 1)[1].split()
                if int(fields[2]) == group:
                    count += 1
            except (OSError, ValueError, IndexError):
                continue
    return count


def child_limits(timeout: float) -> None:
    os.umask(0o022)
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_FILE_BYTES, MAX_FILE_BYTES))
    resource.setrlimit(resource.RLIMIT_CPU, (math.ceil(timeout) + 1, math.ceil(timeout) + 2))
    # Avoid inheriting an unusually large descriptor limit into fixtures.
    _soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    target = min(256, hard) if hard != resource.RLIM_INFINITY else 256
    resource.setrlimit(resource.RLIMIT_NOFILE, (target, hard))


def capture(command: list[str], cwd: Path, environment: dict[str, str], timeout: float,
            merged: bool = False) -> tuple[bytes, bytes, int, str | None, bool]:
    """Bound output and execution; kill the entire inherited group on every exit."""
    process = subprocess.Popen(command, cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT if merged else subprocess.PIPE,
                               start_new_session=True, close_fds=True,
                               preexec_fn=lambda: child_limits(timeout))
    selector = selectors.DefaultSelector()
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    for name, stream in (("stdout", process.stdout), ("stderr", process.stderr)):
        if stream is not None:
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
    deadline = time.monotonic() + timeout
    last_child_check = 0.0
    limit: str | None = None
    leftovers = False
    kill_deadline: float | None = None
    try:
        while selector.get_map() or process.poll() is None:
            now = time.monotonic()
            if limit is None and now >= deadline:
                limit = "timeout"
            if limit is None and now - last_child_check >= 0.1:
                last_child_check = now
                if group_size(process.pid) > MAX_CHILDREN:
                    limit = "child-count"
            if process.poll() is not None and kill_deadline is None:
                leftovers = process_group_exists(process.pid)
                kill_group(process.pid)
                kill_deadline = now + 0.5
            if limit is not None and kill_deadline is None:
                kill_group(process.pid)
                kill_deadline = now + 0.5
            if kill_deadline is not None and now >= kill_deadline:
                break
            for key, _events in selector.select(0.02):
                chunk = os.read(key.fd, 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                room = MAX_OUTPUT - sum(len(value) for value in buffers.values())
                buffers[key.data].extend(chunk[:max(0, room)])
                if len(chunk) > room:
                    limit = "output"
                    kill_group(process.pid)
                    kill_deadline = time.monotonic() + 0.5
        kill_group(process.pid)
        try:
            returncode = process.wait(timeout=1)
        except subprocess.TimeoutExpired as error:
            raise HarnessError("cannot reap fixture process after SIGKILL") from error
    finally:
        kill_group(process.pid)
        for key in list(selector.get_map().values()):
            key.fileobj.close()
        selector.close()
        if process.poll() is None:
            process.wait(timeout=1)
    return bytes(buffers["stdout"]), bytes(buffers["stderr"]), returncode, limit, leftovers


def execute(case: Case, root: Path, executable: Path) -> Observation:
    reset_workspace(case, root, executable)
    out, err, status, limit, leftovers = capture(
        [str(root / "bin" / "make"), *case.args], root / "work",
        clean_environment(root, case.environment), case.timeout, case.output == "merged")
    files = snapshot({name: root / name for name in ("work", "home", "tmp")})
    return Observation(out, err, status, limit, files, leftovers)


def differences(reference: Observation, candidate: Observation) -> list[str]:
    mismatch = []
    for field in dataclasses.fields(Observation):
        if getattr(reference, field.name) != getattr(candidate, field.name):
            mismatch.append(field.name)
    # Two resource-limit failures are never evidence of conformance.
    if reference.limit is not None or candidate.limit is not None:
        mismatch.append("execution-limit")
    if reference.leftover_processes or candidate.leftover_processes:
        mismatch.append("background-processes")
    return mismatch


def expectation_errors(case: Case, observation: Observation) -> list[str]:
    errors = []
    if observation.limit is not None:
        errors.append(f"oracle hit {observation.limit} limit")
    if observation.leftover_processes:
        errors.append("oracle left background processes")
    for name, expected in case.expected.items():
        if name.endswith("_hex"):
            actual = getattr(observation, name[:-4])
            expected = bytes.fromhex(expected)
        else:
            actual = observation.returncode if name == "exit_code" else getattr(observation, name)
            if name != "exit_code":
                expected = expected.encode("utf-8")
        if actual != expected:
            errors.append(f"oracle disagrees with expected.{name}")
    return errors


def executable_identity(path: Path, *, oracle: bool = False) -> dict[str, Any]:
    path = path.expanduser().resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise HarnessError(f"executable not found or not executable: {path}")
    with tempfile.TemporaryDirectory(prefix="rmake-identity-") as temporary:
        root = Path(temporary)
        for name in ("bin", "home", "tmp"):
            (root / name).mkdir()
        output, error, status, limit, leftovers = capture(
            [str(path), "--version"], root, clean_environment(root), 5)
    if limit or leftovers or status != 0:
        raise HarnessError(f"unable to obtain executable identity: {path}")
    first_line = output.splitlines()[0] if output.splitlines() else b""
    if oracle and first_line != b"GNU Make 4.4.1":
        raise HarnessError(f"oracle must be GNU Make 4.4.1, got {first_line!r}: {path}")
    return {"path": str(path), "sha256": sha256_file(path),
            "version_stdout": output.decode("utf-8", errors="replace"),
            "version_stderr": error.decode("utf-8", errors="replace")}


def observation_json(observation: Observation) -> dict[str, Any]:
    return {"returncode": observation.returncode, "limit": observation.limit,
            "leftover_processes": observation.leftover_processes,
            "filesystem": observation.filesystem}


def save_artifact(directory: Path, case: Case, root: Path, identities: dict[str, Any],
                  reference: Observation, candidate: Observation,
                  errors: list[str], status: str) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    target = Path(tempfile.mkdtemp(prefix=f"{case.id}-", dir=directory))
    shutil.copytree(case.path, target / "input", symlinks=True)
    report = {"case": case.id, "requirement": case.requirement, "status": status,
              "differences": errors, "identities": identities,
              "argv": [str(root / "bin" / "make"), *case.args],
              "cwd": str(root / "work"), "environment": clean_environment(root, case.environment),
              "output_topology": case.output, "timeout_seconds": case.timeout,
              "limitations": ["trusted-fixture process containment, no security sandbox",
                              "timestamps not compared", "single invocation only"],
              "oracle": observation_json(reference), "candidate": observation_json(candidate)}
    (target / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    for label, observed in (("oracle", reference), ("candidate", candidate)):
        (target / f"{label}.stdout").write_bytes(observed.stdout)
        (target / f"{label}.stderr").write_bytes(observed.stderr)
    return target


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--oracle-profile", type=Path, default=reference_oracle.PROFILE)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--self-test", action="store_true", help="compare GNU oracle to itself")
    mode.add_argument("--candidate", type=Path)
    parser.add_argument("--cases", type=Path, default=ROOT / "tests/cases")
    parser.add_argument("--case", action="append", default=[], help="exact case ID; repeatable")
    parser.add_argument("--repeat", type=int, default=1,
                        help="repeat exactly selected cases in fresh workspaces (1-1000)")
    parser.add_argument("--artifacts", type=Path, default=ROOT / "artifacts/conformance")
    args = parser.parse_args(argv)
    try:
        if not 1 <= args.repeat <= 1000:
            raise HarnessError("repeat must be between 1 and 1000")
        if args.repeat > 1 and not args.case:
            raise HarnessError("repetition requires exact --case selection")
        profile = reference_oracle.verify(args.oracle.expanduser().resolve(), args.oracle_profile)
        oracle_identity = executable_identity(args.oracle, oracle=True)
        oracle_identity["profile"] = profile
        candidate_identity = (oracle_identity if args.self_test else
                              executable_identity(args.candidate))
        oracle = Path(oracle_identity["path"])
        candidate = Path(candidate_identity["path"])
        if not args.cases.is_dir() or args.cases.is_symlink():
            raise HarnessError(f"cases must be a real directory: {args.cases}")
        all_cases = [load_case(path) for path in sorted(args.cases.iterdir()) if path.is_dir()]
        if not all_cases:
            raise HarnessError("no cases found")
        ids = {case.id for case in all_cases}
        if set(args.case) - ids:
            raise HarnessError(f"unknown case IDs: {sorted(set(args.case) - ids)}")
        cases = [case for case in all_cases if not args.case or case.id in args.case]
        print("M0: trusted fixtures only; process-group containment is NOT a security sandbox.")
        print("Comparison: bytes/status/files/type/mode/links; timestamps and stateful steps are not implemented.")
        totals = {"PASS": 0, "PENDING": 0, "FAIL": 0, "XPASS": 0}
        identities = {"oracle": oracle_identity, "candidate": candidate_identity}
        # Deliberately serial: the same alias/workspace is reset between engines.
        with tempfile.TemporaryDirectory(prefix="rmake-conformance-") as temporary:
            root = Path(temporary)
            for attempt in range(args.repeat):
                for case in cases:
                    reference = execute(case, root, oracle)
                    observed = execute(case, root, candidate)
                    expected_errors = expectation_errors(case, reference)
                    mismatch = differences(reference, observed)
                    if expected_errors or reference.limit or observed.limit or reference.leftover_processes or observed.leftover_processes:
                        status = "FAIL"
                    elif args.self_test or case.candidate == "required":
                        status = "FAIL" if mismatch else "PASS"
                    else:
                        status = "PENDING" if mismatch else "XPASS"
                    totals[status] += 1
                    suffix = ", ".join(expected_errors + mismatch)
                    repetition = f" (run {attempt + 1}/{args.repeat})" if args.repeat > 1 else ""
                    print(f"{status:7} {case.id}{repetition}" + (f" ({suffix})" if suffix else ""))
                    if status != "PASS":
                        artifact = save_artifact(args.artifacts, case, root, identities, reference,
                                                 observed, expected_errors + mismatch, status)
                        print(f"        evidence: {artifact}")
        print("Results: " + ", ".join(f"{key.lower()}={value}" for key, value in totals.items()))
        if totals["PENDING"]:
            print("Pending cases are observed mismatches, not compatibility passes.")
        return 1 if totals["FAIL"] or totals["XPASS"] else 0
    except (HarnessError, reference_oracle.OracleError, OSError) as error:
        print(f"conformance: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
