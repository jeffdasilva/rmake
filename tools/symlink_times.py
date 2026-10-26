#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original anchored default/-L stat/lstat/readlink fault programs on Linux/glibc.

Trusted bounded inputs and a syscall interposer; not a security sandbox.
"""
from __future__ import annotations

import argparse
import base64
import dataclasses
import json
from pathlib import Path
import shutil
import sys
import tempfile

import conformance as harness
import oracle
import stdio_faults as faults

ROOT = Path(__file__).resolve().parent.parent
ARGS = ('-rR', '--no-print-directory', '-L')
SOURCE = (b'$(shell python3 setup.py)\nout: effect-probe\n'
          b"\t@printf 'build=%s\\n' '$?'\n")
LIBRARY_SOURCE = SOURCE.replace(b'out: effect-probe', b'.LIBPATTERNS=%\nout: -leffect-probe')
SETUP = (b'import os\n'
         b'for name, seconds in [("input", 0), ("out", 10), ("effect-probe", 20)]:\n'
         b'    os.utime(name, ns=((1600000000 + seconds) * 10**9,) * 2, follow_symlinks=False)\n')
PROTECTED = b'Original symlink timestamp campaign anchor.\n'


@dataclasses.dataclass(frozen=True)
class Cell:
    id: str
    fault: str
    flags: tuple[str, ...]
    stdout: bytes
    stderr: bytes
    status: int
    symlinks: bool = True
    library: bool = False


def inventory() -> tuple[Cell, ...]:
    cells = []
    errors = {'eio': b'Input/output error', 'eacces': b'Permission denied',
              'enoent': b'No such file or directory', 'enotdir': b'Not a directory'}
    for operation, kinds in (('stat', ('eintr', 'eio', 'eacces', 'enoent', 'enotdir')),
                             ('lstat', ('eintr', 'eio', 'eacces', 'enoent', 'enotdir')),
                             ('readlink', ('eintr', 'eio', 'enoent'))):
        for kind in kinds:
            fatal = operation == 'stat' and kind in ('eio', 'eacces')
            current = operation == 'lstat' and kind != 'eintr'
            stderr = b''
            if kind != 'eintr' and not (operation != 'readlink' and kind in ('enoent', 'enotdir')):
                stderr = b'make: ' + operation.encode() + b': effect-probe: ' + errors[kind] + b'\n'
            if fatal:
                stderr += b"make: *** No rule to make target 'effect-probe', needed by 'out'.  Stop.\n"
            for mode, flags in (('normal', ()), ('dry', ('-n',)), ('query', ('-q',)), ('touch', ('-t',))):
                if fatal:
                    stdout, status = b'', 2
                elif mode == 'query':
                    stdout, status = b'', 0 if current else 1
                elif current:
                    stdout, status = b"make: 'out' is up to date.\n", 0
                else:
                    stdout, status = {'normal': b'build=effect-probe\n',
                                      'dry': b"printf 'build=%s\\n' 'effect-probe'\n",
                                      'touch': b'touch out\n'}[mode], 0
                cells.append(Cell(operation + '-' + kind + '-' + mode,
                                  'symlink-' + operation + '-' + kind, flags, stdout, stderr, status))
    # Ordinary stat errors use the same real input and protected filesystem
    # anchors, with link-time traversal disabled. Only EINTR is retried;
    # ENOENT/ENOTDIR are silent misses, all other errors are diagnosed.
    for kind in ('eintr', 'eio', 'eacces', 'enoent', 'enotdir'):
        stderr = b''
        if kind in ('eio', 'eacces'):
            stderr = b'make: stat: effect-probe: ' + errors[kind] + b'\n'
        if kind != 'eintr':
            stderr += b"make: *** No rule to make target 'effect-probe', needed by 'out'.  Stop.\n"
        for mode, flags in (('normal', ()), ('dry', ('-n',)), ('query', ('-q',)), ('touch', ('-t',))):
            stdout = b"make: 'out' is up to date.\n" if kind == 'eintr' and mode != 'query' else b''
            cells.append(Cell('default-stat-' + kind + '-' + mode,
                              'symlink-stat-' + kind, flags, stdout, stderr,
                              0 if kind == 'eintr' else 2, False))
    ordinary_and_link_stats = [cell for cell in cells
                               if cell.id.startswith(('stat-', 'default-stat-'))]
    for cell in ordinary_and_link_stats:
        cells.append(dataclasses.replace(
            cell, id='library-' + cell.id, library=True,
            stderr=cell.stderr.replace(b"target 'effect-probe', needed", b"target '-leffect-probe', needed")))
    return tuple(cells)


def inputs(cell: Cell | None = None) -> dict:
    return {'Makefile': faults.regular_file(LIBRARY_SOURCE if cell and cell.library else SOURCE), 'setup.py': faults.regular_file(SETUP),
            'input': faults.regular_file(b'input\n'), 'out': faults.regular_file(b'old\n'),
            'effect-probe': {'type': 'symlink', 'mode': 0o777, 'target': 'input'},
            'protected.txt': faults.regular_file(PROTECTED)}


def materialize(cell: Cell, parent: Path, library: Path) -> harness.Case:
    path = parent / cell.id
    path.mkdir(parents=True)
    for name, entry in inputs(cell).items():
        target = path / name
        if entry['type'] == 'symlink':
            target.symlink_to(entry['target'])
        else:
            target.write_bytes(base64.b64decode(entry['content']))
            target.chmod(0o644)
    return harness.Case(path, cell.id, 'filesystem.symlink-times', 'required', (ARGS if cell.symlinks else ARGS[:-1]) + cell.flags,
                        5, {'LD_PRELOAD': str(library), 'RMAKE_STDIO_FAULT': cell.fault}, {})


def expected(cell: Cell) -> harness.Observation:
    files = {name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp', 'work')}
    files.update({'work/' + name: entry for name, entry in inputs(cell).items()})
    files['work/fault.log'] = faults.regular_file(b'symlink-fault\n')
    return harness.Observation(cell.stdout, cell.stderr, cell.status, None, files)


def run_campaign(cells, reference, candidate, artifacts, identities, replay):
    records = []
    with tempfile.TemporaryDirectory(prefix='rmake-symlink-times-') as temporary:
        root = Path(temporary)
        workspace = root / 'execution'
        workspace.mkdir()
        library, helper = faults.compile_helper(root / 'helper')
        for cell in cells:
            case = materialize(cell, root / 'inputs', library)
            anchor = expected(cell)
            observed = {label: harness.execute(case, workspace, binary)
                        for label, binary in (('oracle', reference), ('candidate', candidate))}
            errors = []
            for label, value in observed.items():
                errors.extend(label + ': authored ' + error for error in harness.differences(anchor, value))
            errors.extend(harness.differences(observed['oracle'], observed['candidate']))
            record = {'case': cell.id, 'fault': cell.fault, 'status': 'FAIL' if errors else 'PASS',
                      'expected': harness.observation_json(anchor)}
            if errors:
                artifact = harness.save_artifact(artifacts, case, workspace, identities,
                                                 observed['oracle'], observed['candidate'], errors, 'FAIL')
                shutil.copyfile(faults.SOURCE, artifact / faults.SOURCE.name)
                record.update(errors=errors, artifact=str(artifact), replay_argv=replay + ['--case', cell.id])
                (artifact / 'symlink.json').write_text(json.dumps(record, indent=2, sort_keys=True) + '\n')
            records.append(record)
            print(record['status'] + ' ' + cell.id + (': ' + ', '.join(errors) if errors else ''), flush=True)
    failures = sum(record['status'] == 'FAIL' for record in records)
    report = {'result': 'FAIL' if failures else 'PASS', 'passed': len(cells) - failures,
              'failed': failures, 'executed': len(cells), 'identities': identities,
              'helper': helper, 'cases': records, 'replay_argv': replay,
              'limits': 'trusted bounded Makefiles; no security sandbox; timestamps outside this gate'}
    campaigns = artifacts / 'campaigns'
    campaigns.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', prefix='symlink-', dir=campaigns,
                                     delete=False) as file:
        json.dump(report, file, indent=2, sort_keys=True)
        file.write('\n')
        report_path = file.name
    print(f'Symlink timestamps: pass={len(cells) - failures}, fail={failures}')
    print(f'Campaign report: {report_path}')
    return 1 if failures else 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--self-test', action='store_true')
    mode.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--oracle-profile', type=Path, default=oracle.PROFILE)
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--artifacts', type=Path, default=ROOT / 'artifacts/symlink-times')
    args = parser.parse_args(argv)
    try:
        cells = inventory()
        if set(args.case) - {cell.id for cell in cells}:
            raise harness.HarnessError('unknown symlink timestamp case')
        cells = tuple(cell for cell in cells if not args.case or cell.id in args.case)
        reference = harness.executable_identity(args.oracle, oracle=True)
        reference['profile'] = oracle.verify(Path(reference['path']), args.oracle_profile)
        candidate = reference if args.self_test else harness.executable_identity(args.candidate)
        replay = [sys.executable, str(Path(__file__).resolve()), '--oracle', reference['path'],
                  '--oracle-profile', str(args.oracle_profile.expanduser().resolve()),
                  *(['--self-test'] if args.self_test else ['--candidate', candidate['path']])]
        return run_campaign(cells, Path(reference['path']), Path(candidate['path']),
                            args.artifacts.expanduser().resolve(),
                            {'oracle': reference, 'candidate': candidate}, replay)
    except (harness.HarnessError, oracle.OracleError, OSError) as error:
        print(f'symlink-times: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
