#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare ordinary grouped prerequisite diagnostics while their recipes wait."""
from __future__ import annotations

import argparse
from dataclasses import replace
import json
import os
from pathlib import Path
import stat
import tempfile
import tomllib

import debug_output as debug
from debug_output import h, reference_oracle
from job_debug import proc_identity
from remake_parallel import HeldModel, execute, release_errors

SOURCES = debug.ROOT / 'tests/debug-group-peers'
CONTROLLER = b'Using jobserver controller '
PIPE = b'3,4'
FIFO = b'<verified-fifo>'


class ControllerOutput:
    def __init__(self, runtime: Path, style: str):
        self.runtime = runtime
        self.style = style
        self.identity = None
        self.line = None

    def observe(self, proc: Path, stdout: bytes) -> bytes:
        parent = int(proc.name)
        identity = (parent, proc_identity(parent))
        if self.identity not in (None, identity):
            raise h.HarnessError('held make process identity changed')
        self.identity = identity
        if self.style == 'fifo':
            path = self.runtime / 'tmp' / f'GMfifo{parent}'
            if not stat.S_ISFIFO(path.lstat().st_mode):
                raise h.HarnessError('jobserver controller is not the live FIFO')
            value = b'fifo:' + os.fsencode(path)
        else:
            read, write = (os.stat(proc / 'fd' / str(fd)) for fd in (3, 4))
            if not stat.S_ISFIFO(read.st_mode) or not os.path.samestat(read, write):
                raise h.HarnessError('jobserver descriptors are not the live pipe')
            value = PIPE
        self.line = CONTROLLER + value + b'\n'
        return self.normalize(stdout)

    def normalize(self, stdout: bytes) -> bytes:
        if self.line is None or stdout.count(self.line) != 1:
            raise h.HarnessError('stdout does not identify the observed jobserver')
        canonical = FIFO if self.style == 'fifo' else PIPE
        return stdout.replace(self.line, CONTROLLER + canonical + b'\n', 1)


def models() -> list[tuple[str, HeldModel]]:
    data = tomllib.loads((SOURCES / 'models.toml').read_text())
    if data['schema_version'] != 1:
        raise h.HarnessError('unsupported grouped-peer model schema')
    worker = (SOURCES / 'worker.py').read_bytes()
    result = []
    for row in data['models']:
        for style, controller in (('pipe', PIPE), ('fifo', FIFO)):
            header = data['stdout_header'].format(controller=controller.decode()).encode()
            body = row['stdout'].encode().splitlines(keepends=True)
            cuts = row['release_lines']
            if not 1 <= len(cuts) <= 2 or cuts != sorted(set(cuts)) or not 0 < cuts[0] <= cuts[-1] <= len(body):
                raise h.HarnessError('invalid grouped-peer release boundaries')
            prefixes = [header + b''.join(body[:lines]) for lines in cuts]
            files = {'Makefile': row['makefile'].encode(), 'worker.py': worker, 'seen': b'anchor\n'}
            times = {}
            if 'source' in row:
                files['source'] = row['source'].encode()
                times['source'] = 1_000_000_000_000_000_000
            overlap = len(prefixes) == 2
            workers = ('left', 'right') if overlap else ('left',)
            updates = {f'{name}.{suffix}': value for name in workers for suffix, value in (
                ('started', b'started\n'), ('release', b'yes\n'), ('done', b'done\n'))}
            updates.update({name: value.encode() for name, value in row.get('outputs', {}).items()})
            model = debug.Model(
                row['id'] + '-' + style, files,
                ('-rR', '--no-print-directory', '--debug=v', '-j2', f'--jobserver-style={style}',
                 *row['flags'], *row.get('goals', ['all'])), {},
                {'exit_code': row['exit_code'], 'stdout_template_hex': (header + b''.join(body)).hex(),
                 'stderr_template_hex': row['stderr'].encode().hex()},
                row['seen'].encode(), updates=updates, mtimes_ns=times,
            )
            result.append((style, HeldModel(model, overlap, b'anchor\nleft\n' if overlap else None,
                                            before_first=b'anchor\n', before_first_stdout=prefixes[0],
                                            before_second_stdout=prefixes[1] if overlap else None)))
    if not result or len({held.model.id for _, held in result}) != len(result):
        raise h.HarnessError('grouped-peer models must be nonempty and uniquely named')
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=reference_oracle.PREFIX / 'bin' / 'make')
    parser.add_argument('--oracle-profile', type=Path, default=reference_oracle.PROFILE)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--oracle-only', action='store_true')
    mode.add_argument('--candidate', type=Path)
    parser.add_argument('--report-dir', type=Path, required=True)
    args = parser.parse_args()
    profile = reference_oracle.verify(args.oracle, args.oracle_profile)
    binaries = [args.oracle, args.oracle]
    if args.candidate is not None:
        binaries.append(args.candidate)
    records = []
    with tempfile.TemporaryDirectory(prefix='rmake-debug-group-peers-') as scratch:
        root = Path(scratch)
        runtime = root / 'runtime'
        runtime.mkdir(mode=0o755)
        for style, held in models():
            model = held.model
            case, filesystem = debug.prepare(model, root / model.id, runtime)
            observed = []
            raw_stdout = []
            releases = []
            for binary in binaries:
                controller = ControllerOutput(runtime, style)
                observation, release = execute(case, runtime, binary, wait_stdout=controller.observe)
                raw_stdout.append(observation.stdout.hex())
                observed.append(replace(observation, stdout=controller.normalize(observation.stdout)))
                releases.append(release)
            record = debug.assess(model, case, runtime, filesystem, observed)
            errors = [release_errors(held, release) for release in releases]
            record.update(id=model.id, releases=releases, release_errors=errors,
                          raw_stdout_hex=raw_stdout, passed=record['passed'] and not any(errors))
            records.append(record)
            print('PASS' if record['passed'] else 'FAIL', model.id, flush=True)
    passed = sum(record['passed'] for record in records)
    args.report_dir.mkdir(parents=True, exist_ok=True)
    report = Path(tempfile.mkdtemp(prefix='run-', dir=args.report_dir)) / 'report.json'
    with report.open('x', encoding='utf-8') as output:
        json.dump({'oracle_profile': profile['profile_id'], 'executables': [
            h.executable_identity(binary) for binary in binaries],
            'cases': records, 'passed': passed, 'total': len(records)}, output, indent=2)
        output.write('\n')
    print(f'Grouped prerequisite debug: {passed}/{len(records)} ({report})')
    return int(passed != len(records))


if __name__ == '__main__':
    raise SystemExit(main())
