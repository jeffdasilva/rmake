#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare the capture EOF/reap boundary with an independent pipe witness."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import oracle

ROOT = Path(h.__file__).resolve().parents[1]
WITNESS = ROOT / 'tests/shell-capture/witness.py'


def file_entry(content):
    return {'type': 'file', 'mode': 0o644, 'size': len(content),
            'sha256': hashlib.sha256(content).hexdigest(),
            'content': base64.b64encode(content).decode()}


def models():
    common = (
        b'status := $(.SHELLSTATUS)\n'
        b'$(file >>seen,expanded status=$(status))\n'
        b'.PHONY: all\n'
        b'all:;@printf "<%s> status=%s\\n" "$(captured)" "$(status)"\n'
    )
    eof = b'before\nchild:saw-reader\nchild:writer-closed\nchild:reader-closed\nexpanded status=7\n'
    descendant = b'before\nchild:main-writer\nchild:descendant-wrote\nexpanded status=9\n'
    for name, expansion, stdout, journal in (
        ('shell-function', b'captured := $(shell python3 witness.py eof)', b'<left right> status=7\n', eof),
        ('shell-assignment', b'captured != python3 witness.py eof', b'<left right> status=7\n', eof),
        ('descendant-writer', b'captured := $(shell python3 witness.py descendant)',
         b'<first late> status=9\n', descendant),
    ):
        yield name, b'$(file >seen,before)\n' + expansion + b'\n' + common, stdout, journal


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--declarations', type=Path)
    args = parser.parse_args()
    profile = oracle.verify(args.oracle.resolve())
    paths = [args.oracle.resolve(), args.oracle.resolve()]
    if args.candidate:
        paths.append(args.candidate.resolve())
    report = {'classification': 'Exact Linux capture descriptor lifetime and original shell status',
              'oracle_profile': profile['profile_id'], 'identities': [h.executable_identity(path) for path in paths],
              'source_sha256': h.sha256_file(Path(__file__)), 'witness_sha256': h.sha256_file(WITNESS),
              'affinity': sorted(os.sched_getaffinity(0)), 'cases': []}
    with tempfile.TemporaryDirectory(prefix='rmake-shell-capture-') as directory:
        root = Path(directory)
        runtime = root / 'runtime'
        runtime.mkdir()
        prepared = []
        for name, makefile, stdout, journal in models():
            fixture = root / name
            fixture.mkdir()
            (fixture / 'Makefile').write_bytes(makefile)
            (fixture / 'witness.py').write_bytes(WITNESS.read_bytes())
            case = h.Case(fixture, name, 'shell.capture.lifetime', 'required',
                          ('-rR', '--no-print-directory'), 6, {},
                          {'exit_code': 0, 'stdout_hex': stdout.hex(), 'stderr_hex': ''})
            h.reset_workspace(case, runtime, paths[0])
            expected = h.snapshot({part: runtime / part for part in ('work', 'home', 'tmp')})
            expected['work/seen'] = file_entry(journal)
            prepared.append((case, {'id': name, 'args': case.args, 'expected': case.expected,
                                    'expected_filesystem': expected, 'expected_seen_hex': journal.hex()}))
        if args.declarations:
            with args.declarations.open('x') as stream:
                json.dump([row for _, row in prepared], stream, indent=2)
        for case, declaration in prepared:
            row = {**declaration, 'observations': []}
            for executable in paths:
                value = h.execute(case, runtime, executable)
                errors = h.expectation_errors(case, value)
                if value.filesystem != row['expected_filesystem']:
                    errors.append('complete filesystem')
                row['observations'].append({**h.observation_json(value), 'stdout_hex': value.stdout.hex(),
                                            'stderr_hex': value.stderr.hex(), 'differences': errors})
                if value.limit or value.leftover_processes or value.returncode < 0:
                    row['zero_credit'] = True
                    break
            row['passed'] = not any(run['differences'] for run in row['observations'])
            report['cases'].append(row)
            print('PASS' if row['passed'] else 'FAIL', row['id'],
                  [run['differences'] for run in row['observations']], flush=True)
            if row.get('zero_credit'):
                break
    report['totals'] = {'cases': len(report['cases']), 'pass': sum(row['passed'] for row in report['cases'])}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return int(report['totals']['pass'] != len(prepared))


if __name__ == '__main__':
    raise SystemExit(main())
