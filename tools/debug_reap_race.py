#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact early-child-reap regression using only an owned stdout pipe and procfs."""
from __future__ import annotations

import argparse
from array import array
import fcntl
import json
import os
from pathlib import Path
import selectors
import subprocess
import tempfile
import termios
import time
import tomllib

import debug_output as debug
from debug_output import h
from job_debug import proc_identity

SOURCES = debug.ROOT / 'tests/debug-reap-race'
INNER_WAIT = b" The prerequisites of 'middle' are being made.\n"
OUTER_WAIT = b"The prerequisites of 'all' are being made.\n"
MAX_OUTPUT = 64 * 1024


def models() -> list[tuple[debug.Model, int]]:
    data = tomllib.loads((SOURCES / 'models.toml').read_text())
    if data['schema_version'] != 1:
        raise h.HarnessError('unsupported reap race model schema')
    worker = (SOURCES / 'worker.py').read_bytes()
    result = []
    for row in data['models']:
        output = (data['stdout_prefix'] + row['stdout']).encode()
        if output.count(INNER_WAIT) != 1 or output.count(OUTER_WAIT) != 1:
            raise h.HarnessError('reap race model must declare one inner and outer wait')
        model = debug.Model(
            row['id'], {'Makefile': row['makefile'].encode(), 'worker.py': worker,
                        'all': b'present\n', 'middle': b'present\n', 'seen': b'anchor\n'},
            ('-rR', '--no-print-directory', '--debug=v', '-s', '-j2', '--jobserver-style=pipe', 'all'),
            {'GNUMAKEFLAGS': ''},
            {'exit_code': row['exit_code'], 'stdout_template_hex': output.hex(),
             'stderr_template_hex': row['stderr'].encode().hex()}, b'anchor\n',
            updates={'started': b'started\n', 'release': b'yes\n', 'done': b'done\n'},
        )
        result.append((model, row['recipe_status']))
    if not result or len({model.id for model, _ in result}) != len(result):
        raise h.HarnessError('reap race models must be nonempty and uniquely named')
    return result


def bytes_waiting(fd: int) -> int:
    count = array('i', [0])
    fcntl.ioctl(fd, termios.FIONREAD, count, True)
    return count[0]


def await_condition(process: subprocess.Popen, deadline: float, description: str, check):
    while True:
        found = check()
        if found is not None:
            return found
        if process.poll() is not None or time.monotonic() >= deadline:
            raise h.HarnessError(f'{description} was not observed before the execution bound')
        time.sleep(.001)


def find_worker(parent: int, status: int) -> tuple[int, int] | None:
    children = Path('/proc', str(parent), 'task', str(parent), 'children')
    for token in children.read_bytes().split():
        pid = int(token)
        try:
            command = Path('/proc', str(pid), 'cmdline').read_bytes().split(b'\0')
            if command[1:] != [b'worker.py', str(status).encode(), b'']:
                continue
            ppid, start = proc_identity(pid)
            if ppid == parent and start > 0:
                return pid, start
        except (FileNotFoundError, ProcessLookupError):
            continue
    return None


def release_waitable_worker(process: subprocess.Popen, work: Path, reader: int,
                            capacity: int, status: int, deadline: float) -> dict:
    def ready():
        if bytes_waiting(reader) != capacity or not (work / 'started').is_file():
            return None
        return find_worker(process.pid, status)

    worker, start = await_condition(process, deadline, 'the full pipe and live recipe child', ready)
    if (work / 'started').read_bytes() != b'started\n' or (work / 'done').exists():
        raise h.HarnessError('recipe child escaped its declared release barrier')
    release = work / 'release.tmp'
    release.write_bytes(b'yes\n')
    release.chmod(0o644)
    release.rename(work / 'release')

    def zombie():
        try:
            ppid, identity = proc_identity(worker)
            state = Path('/proc', str(worker), 'stat').read_bytes().rsplit(b') ', 1)[1].split()[0]
        except (FileNotFoundError, ProcessLookupError) as error:
            raise h.HarnessError('recipe child vanished before stdout was released') from error
        if (ppid, identity) != (process.pid, start):
            raise h.HarnessError('recipe child identity changed during the barrier')
        return state if state == b'Z' else None

    await_condition(process, deadline, 'the owned waitable recipe child', zombie)
    if bytes_waiting(reader) != capacity or (work / 'done').read_bytes() != b'done\n':
        raise h.HarnessError('the pipe or recipe completion changed during the barrier')
    return {'parent': process.pid, 'worker': worker, 'worker_starttime': start,
            'worker_state': 'Z', 'held_pipe_bytes': capacity}


def collect_output(process: subprocess.Popen, reader: int, deadline: float) -> tuple[bytes, bytes]:
    buffers = [bytearray(), bytearray()]
    with selectors.DefaultSelector() as selector:
        for index, fd in enumerate((reader, process.stderr.fileno())):
            os.set_blocking(fd, False)
            selector.register(fd, selectors.EVENT_READ, index)
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise h.HarnessError('reap race stream capture exceeded the execution bound')
            for key, _events in selector.select(remaining):
                chunk = os.read(key.fd, 4096)
                if not chunk:
                    selector.unregister(key.fd)
                    continue
                buffers[key.data].extend(chunk)
                if sum(map(len, buffers)) > MAX_OUTPUT:
                    raise h.HarnessError('reap race output exceeded its byte bound')
    return bytes(buffers[0]), bytes(buffers[1])


def execute(model: debug.Model, case: h.Case, runtime: Path, binary: Path,
            recipe_status: int) -> tuple[h.Observation, dict]:
    h.reset_workspace(case, runtime, binary)
    work = runtime / 'work'
    expected = bytes.fromhex(model.expected['stdout_template_hex'])
    prefix = expected.partition(INNER_WAIT)[0]
    read_fd, write_fd = os.pipe2(os.O_CLOEXEC)
    with os.fdopen(read_fd, 'rb', buffering=0) as reader, os.fdopen(write_fd, 'wb', buffering=0) as writer:
        # Fill one anonymous pipe page, reserving exactly the literal output
        # before the inner wait. No engine byte can pass this boundary until
        # the supervisor drains the prefix after the child becomes waitable.
        try:
            page = os.sysconf('SC_PAGESIZE')
            capacity = fcntl.fcntl(writer, fcntl.F_SETPIPE_SZ, page)
        except (AttributeError, OSError) as error:
            raise h.HarnessError('reap race requires unprivileged Linux pipe sizing') from error
        if capacity != page or not 0 < len(prefix) < capacity <= MAX_OUTPUT:
            raise h.HarnessError('reap race requires a one-page stdout pipe larger than the prefix')
        padding = b'\0' * (capacity - len(prefix))
        if writer.write(padding) != len(padding):
            raise h.HarnessError('could not seed the owned stdout pipe')
        process = subprocess.Popen(
            [str(runtime / 'bin/make'), *case.args], cwd=work, env=h.clean_environment(runtime, case.environment),
            stdin=subprocess.DEVNULL, stdout=writer, stderr=subprocess.PIPE, start_new_session=True,
            close_fds=True, preexec_fn=lambda: h.child_limits(case.timeout),
        )
        leftovers = False
        try:
            writer.close()
            deadline = time.monotonic() + case.timeout
            witness = release_waitable_worker(process, work, read_fd, capacity, recipe_status, deadline)
            stdout, stderr = collect_output(process, read_fd, deadline)
            status = process.wait(timeout=max(.001, deadline - time.monotonic()))
            leftovers = h.process_group_exists(process.pid)
        finally:
            if process.returncode is None or leftovers:
                h.kill_group(process.pid)
            process.stderr.close()
            if process.poll() is None:
                process.wait(timeout=1)
        # These bytes were inserted by this test before make could execute.
        # Every engine-produced byte, including the held prefix, is compared.
        if not stdout.startswith(padding):
            raise h.HarnessError('stdout pipe did not retain its test-owned transport prefix')
        stdout = stdout[len(padding):]
        witness['engine_prefix_hex'] = stdout[:len(prefix)].hex()
        witness['engine_prefix_exact'] = stdout.startswith(prefix)
    files = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    return h.Observation(stdout, stderr, status, None, files, leftovers), witness


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=debug.ROOT / '.tools/oracle/bin/make')
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--oracle-only', action='store_true')
    mode.add_argument('--candidate', type=Path)
    parser.add_argument('--report-dir', type=Path, required=True)
    parser.add_argument('--repeat', type=int, default=1)
    args = parser.parse_args()
    if not 1 <= args.repeat <= 8:
        parser.error('--repeat must be between 1 and 8')
    report = {'passed': False, 'records': [], 'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
              'effective_uid': os.geteuid()}
    try:
        selected = models()
        profile = debug.reference_oracle.verify(args.oracle.resolve())
        oracle = args.oracle.resolve()
        identities = {'oracle': h.executable_identity(oracle, oracle=True)}
        binaries = [oracle, oracle]
        if args.candidate is not None:
            identities['candidate'] = h.executable_identity(args.candidate)
            binaries.append(args.candidate.resolve())
        report.update(identities=identities, oracle_profile_id=profile['profile_id'],
                      source_sha256={p.name: h.sha256_file(p) for p in (
                          Path(__file__), SOURCES / 'models.toml', SOURCES / 'worker.py',
                          Path(debug.__file__), Path(h.__file__))})
        with tempfile.TemporaryDirectory(prefix='rmake-debug-reap-race-') as folder:
            root = Path(folder)
            runtime = root / 'runtime'
            runtime.mkdir()
            for model, status in selected:
                case, expected_files = debug.prepare(model, root / model.id, runtime)
                for repeat in range(args.repeat):
                    observed, witnesses = zip(*(execute(model, case, runtime, binary, status) for binary in binaries))
                    judgment = debug.assess(model, case, runtime, expected_files, list(observed))
                    judgment['passed'] &= all(witness['engine_prefix_exact'] for witness in witnesses)
                    report['records'].append({'id': model.id, 'repeat': repeat, 'witnesses': witnesses, **judgment})
                    print(('PASS ' if judgment['passed'] else 'FAIL ') + model.id, flush=True)
        report['passed'] = all(row['passed'] for row in report['records'])
    except (KeyError, OSError, ValueError, subprocess.SubprocessError, h.HarnessError,
            debug.reference_oracle.OracleError) as error:
        report['error'] = str(error)
        print('ERROR ' + str(error), flush=True)
    args.report_dir.mkdir(parents=True, exist_ok=True)
    report_path = Path(tempfile.mkdtemp(prefix='run-', dir=args.report_dir)) / 'report.json'
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    print(f'Reap race: {sum(row["passed"] for row in report["records"])}/{len(report["records"])} ({report_path})')
    return int(not report['passed'])


if __name__ == '__main__':
    raise SystemExit(main())
