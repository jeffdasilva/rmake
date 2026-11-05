#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare complete GNU2/rmake databases and original prerequisite role models."""
from __future__ import annotations

import argparse
import base64
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
import tomllib
from typing import Any

import conformance as harness
import database_output as database
import oracle as reference

ROOT = Path(__file__).resolve().parents[1]
MODELS = ROOT / 'tests/database-order-roles/models.toml'
CLOCK = re.compile(rb'(?m)^# (Make data base, printed on|Finished Make data base on) ([^\n]{24})$')
FIXED_NS = 1_600_000_000_000_000_000


@dataclass(frozen=True)
class Model:
    id: str
    makefile: bytes
    records: tuple[bytes, ...]
    seen: bytes | None
    flags: tuple[str, ...]
    status: int
    existing: bool
    explicit_goal: bool


def models() -> list[Model]:
    raw = tomllib.loads(MODELS.read_text())
    if raw['schema_version'] != 1:
        raise harness.HarnessError('unsupported database order-role fixture version')
    result = [Model(row['id'], row['makefile'].encode(), tuple(s.encode() for s in row['records']),
                    bytes.fromhex(row['seen_hex']) if 'seen_hex' in row else None,
                    tuple(row.get('flags', [])), row.get('status', 0), row.get('existing', False),
                    row.get('explicit_goal', True)) for row in raw['case']]
    ids = [model.id for model in result]
    if len(set(ids)) != len(ids) or any(not re.fullmatch(r'[a-z0-9-]+', name) for name in ids):
        raise harness.HarnessError('unsafe or duplicate order-role fixture ID')
    return result


def fixture(model: Model, path: Path) -> harness.Case:
    path.mkdir()
    inputs = {name: name.encode() + b'\n' for name in ('n', 'z', 'gate', 'old')}
    inputs['Makefile'] = model.makefile
    if model.existing:
        inputs['all'] = b'existing target\n'
    for name, content in inputs.items():
        (path / name).write_bytes(content)
    goals = ('all',) if model.explicit_goal else ()
    args = ('-rR', '-s', '--no-print-directory', '-f', 'Makefile', *model.flags, *goals)
    return harness.Case(path, model.id, 'database.order-only-duplicate', 'required', args, 15, {}, {})


def validate_stdout(model: Model, output: bytes, start: int, end: int) -> bytes:
    clocks = list(CLOCK.finditer(output))
    if [clock[1] for clock in clocks] != [b'Make data base, printed on', b'Finished Make data base on']:
        raise harness.HarnessError('expected a complete late-selected database')
    opened, closed = (database.parse_clock(clock[2]) for clock in clocks)
    if not start <= opened <= closed <= end or b'(device ' in output:
        raise harness.HarnessError('unexpected ctime or directory identity')
    header, footer = b'\n# Files\n', b'\n# files hash-table stats:\n'
    if output.count(header) != 1 or output.count(footer) != 1 or output.index(header) > output.index(footer):
        raise harness.HarnessError('invalid file database section')
    body = output[output.index(header) + len(header):output.index(footer)]
    records = tuple(line for line in body.splitlines() if line.startswith((b'all:', b'unused:')))
    if records != model.records:
        raise harness.HarnessError(f'prerequisite roles or recorded declaration order differ: {records!r}')
    return CLOCK.sub(lambda clock: b'# ' + clock[1] + b' <checked libc ctime>', output)


def observe(model: Model, case: harness.Case, runtime: Path, executable: Path) -> dict[str, Any]:
    harness.reset_workspace(case, runtime, executable)
    work = runtime / 'work'
    for path in work.iterdir():
        stamp = FIXED_NS + (10_000_000_000 if path.name == 'all' else 0)
        os.utime(path, ns=(stamp, stamp))
    roots = {name: runtime / name for name in ('work', 'home', 'tmp')}
    expected = harness.snapshot(roots)
    if model.seen is not None:
        expected['work/seen'] = {'type': 'file', 'mode': 0o644, 'size': len(model.seen),
                                 'content': base64.b64encode(model.seen).decode(),
                                 'sha256': hashlib.sha256(model.seen).hexdigest()}
    start = database.libc_time()
    argv = [str(runtime / 'bin/make'), *case.args]
    output, error, status, limit, leftovers = harness.capture(argv, work, harness.clean_environment(runtime), case.timeout)
    end = database.libc_time()
    filesystem = harness.snapshot(roots)
    result = {'argv': argv, 'stdout_hex': output.hex(), 'stderr_hex': error.hex(), 'status': status,
              'limit': limit, 'leftovers': leftovers, 'filesystem': filesystem, 'expected_filesystem': expected,
              'libc_time_window': [start, end]}
    try:
        if (status, error, limit, leftovers, filesystem) != (model.status, b'', None, False, expected):
            raise harness.HarnessError('unexpected status, stderr, process bound, or recipe effect')
        stable = validate_stdout(model, output, start, end)
        result['positioned_stdout_sha256'] = hashlib.sha256(stable).hexdigest()
    except (harness.HarnessError, ValueError) as error:
        result['error'] = str(error)
    return result


def run(selected: list[Model], engines: list[Path]) -> list[dict[str, Any]]:
    result = []
    with tempfile.TemporaryDirectory(prefix='rmake-database-order-roles-') as scratch:
        root = Path(scratch)
        runtime = root / 'runtime'
        runtime.mkdir()
        for model in selected:
            case = fixture(model, root / model.id)
            observations = [observe(model, case, runtime, engine) for engine in engines]
            first = observations[0].get('positioned_stdout_sha256')
            passed = first is not None and all(item.get('positioned_stdout_sha256') == first for item in observations)
            result.append({'id': model.id, 'passed': passed, 'observations': observations})
            print(f"{'PASS' if passed else 'FAIL'} {model.id}", flush=True)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--oracle-profile', type=Path, default=reference.PROFILE)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--candidate', type=Path)
    mode.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/database-order-roles/result.json')
    args = parser.parse_args()
    os.umask(0o022)
    try:
        all_models = models()
        if unknown := set(args.case) - {model.id for model in all_models}:
            raise harness.HarnessError(f'unknown order-role cases: {sorted(unknown)}')
        selected = [model for model in all_models if not args.case or model.id in args.case]
        oracle = args.oracle.resolve()
        profile = reference.verify(oracle, args.oracle_profile)
        identities = [harness.executable_identity(oracle, oracle=True)]
        engines = [oracle, oracle]
        if args.candidate is not None:
            candidate = args.candidate.resolve()
            identities.append(harness.executable_identity(candidate))
            engines.append(candidate)
        results = run(selected, engines)
        passed = all(item['passed'] for item in results)
        document = {'passed': passed, 'profile_id': profile['profile_id'], 'identities': identities,
                    'source_sha256': harness.sha256_file(Path(__file__)), 'models_sha256': harness.sha256_file(MODELS),
                    'results': results}
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(document, indent=2) + '\n')
        print(f'Database prerequisite roles: {sum(row["passed"] for row in results)}/{len(results)}')
        return 0 if passed else 1
    except (harness.HarnessError, reference.OracleError, OSError, ValueError, KeyError) as error:
        parser.exit(2, f'database order roles: {error}\n')


if __name__ == '__main__':
    raise SystemExit(main())
