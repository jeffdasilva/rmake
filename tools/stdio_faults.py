#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original bounded Linux/glibc filesystem fault probes against GNU Make 4.4.1.

The trusted test interposer targets only effect-probe. This uses conformance's
same-alias runner, process containment and byte/file observations; it is not a
security sandbox. The helper injects faults, never supplies Make semantics.
"""
from __future__ import annotations

import argparse
import base64
import dataclasses
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile

import conformance
import oracle

SOURCE = Path(__file__).with_suffix('.c')
INITIAL = b'original-content\n'
PROTECTED = b'Original filesystem fault campaign anchor.\n'
PREFIX = b'$(file >prefix.marker,before)\n$(info begin)\n'
SUFFIX = b'$(file >suffix.marker,after)\nall:;@:\n'
ARGS = ('-rR', '--no-print-directory')


@dataclasses.dataclass(frozen=True)
class Cell:
    id: str
    fault: str
    operation: bytes
    stdout: bytes
    stage: str | None
    final: bytes
    trace: bytes

    @property
    def makefile(self) -> bytes:
        return PREFIX + self.operation + b'\n' + SUFFIX


def inventory() -> tuple[Cell, ...]:
    cells = []
    for operation, expression, final, stdout in (
        ('read', b'$(info data=<$(file <effect-probe)>)', INITIAL,
         b'begin\ndata=<original-content>\n'),
        ('write', b'$(file >effect-probe,changed)', b'changed\n', b'begin\n'),
        ('append', b'$(file >>effect-probe,changed)', INITIAL + b'changed\n', b'begin\n'),
    ):
        cells.append(Cell('open-eintr-' + operation, 'open-eintr', expression,
                          stdout, None, final, b'interrupted\n'))
        cells.append(Cell('close-eio-' + operation, 'close-eio', expression,
                          b'begin\n', 'close', final, b'closed-once\n'))
    for fault in ('read-eintr', 'read-eio'):
        cells.append(Cell(fault, fault, b'$(info data=<$(file <effect-probe)>)',
                          b'begin\ndata=<original-content>\n' if fault == 'read-eintr' else b'begin\n',
                          None if fault == 'read-eintr' else 'read', INITIAL,
                          b'partial-read-fault\n'))
    cells.append(Cell('write-eio', 'write-eio', b'$(file >effect-probe,changed)',
                      b'begin\n', 'write', b'changed', b'write-fault\n'))
    for fault in ('realpath-eintr', 'stat-eintr'):
        cells.append(Cell(fault, fault, b'$(info path=<$(realpath effect-probe)>)',
                          b'begin\npath=<@WORK@/effect-probe>\n', None,
                          INITIAL, b'interrupted\n'))
    return tuple(cells)


def regular_file(content: bytes) -> dict:
    return {'type': 'file', 'mode': 0o644, 'size': len(content),
            'content': base64.b64encode(content).decode('ascii'),
            'sha256': hashlib.sha256(content).hexdigest()}


def expected(cell: Cell, workspace: Path) -> conformance.Observation:
    files = {'Makefile': cell.makefile, 'effect-probe': cell.final,
             'protected.txt': PROTECTED, 'prefix.marker': b'before\n', 'fault.log': cell.trace}
    if cell.stage is None:
        files['suffix.marker'] = b'after\n'
    filesystem = {name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp', 'work')}
    filesystem.update({'work/' + name: regular_file(content) for name, content in files.items()})
    stderr = (f'Makefile:3: *** {cell.stage}: effect-probe: Input/output error.  Stop.\n'.encode()
              if cell.stage else b'')
    return conformance.Observation(cell.stdout.replace(b'@WORK@', bytes(workspace / 'work')),
                                   stderr, 2 if cell.stage else 0, None, filesystem)


def materialize(cell: Cell, parent: Path, library: Path) -> conformance.Case:
    path = parent / cell.id
    path.mkdir(parents=True)
    for name, contents in {'Makefile': cell.makefile, 'effect-probe': INITIAL,
                           'protected.txt': PROTECTED}.items():
        (path / name).write_bytes(contents)
        (path / name).chmod(0o644)
    return conformance.Case(path, cell.id, 'effects.filesystem-faults', 'required', ARGS, 5,
                            {'LD_PRELOAD': str(library), 'RMAKE_STDIO_FAULT': cell.fault}, {})


def compile_helper(directory: Path) -> tuple[Path, dict]:
    directory.mkdir()
    library = directory / 'stdio-faults.so'
    compiler = shutil.which('cc', path='/usr/bin:/bin')
    if compiler is None:
        raise conformance.HarnessError('cc is required for Linux/glibc filesystem fault probes')
    command = [compiler, '-shared', '-fPIC', '-O2', '-Wall', '-Wextra', '-Werror',
               '-o', str(library), str(SOURCE.resolve()), '-ldl']
    out, err, status, limit, leftovers = conformance.capture(
        command, directory, conformance.clean_environment(directory), 15)
    if status or limit or leftovers:
        raise conformance.HarnessError(f'fault helper compilation failed: status={status}, limit={limit}, '
                                       f'leftovers={leftovers}: {(out + err).decode(errors="replace")}')
    return library, {'compiler': compiler, 'compiler_sha256': conformance.sha256_file(Path(compiler)),
                     'source_sha256': conformance.sha256_file(SOURCE),
                     'library_sha256': conformance.sha256_file(library), 'compile_argv': command}


def run_campaign(cells: tuple[Cell, ...], reference: Path, candidate: Path,
                 artifacts: Path, identities: dict, replay: list[str]) -> int:
    failures = 0
    records = []
    with tempfile.TemporaryDirectory(prefix='rmake-stdio-faults-') as temporary:
        root = Path(temporary)
        workspace = root / 'execution'
        workspace.mkdir()
        library, helper = compile_helper(root / 'helper')
        for cell in cells:
            case = materialize(cell, root / 'inputs', library)
            anchor = expected(cell, workspace)
            observed = {label: conformance.execute(case, workspace, binary)
                        for label, binary in (('oracle', reference), ('candidate', candidate))}
            errors = []
            for label, result in observed.items():
                errors.extend(label + ': authored ' + difference
                              for difference in conformance.differences(anchor, result))
            errors.extend(conformance.differences(observed['oracle'], observed['candidate']))
            record = {'case': cell.id, 'fault': cell.fault, 'status': 'FAIL' if errors else 'PASS',
                      'expected': {**conformance.observation_json(anchor),
                                   'stdout_hex': anchor.stdout.hex(), 'stderr_hex': anchor.stderr.hex()}}
            if errors:
                failures += 1
                artifact = conformance.save_artifact(artifacts, case, workspace, identities,
                                                       observed['oracle'], observed['candidate'], errors, 'FAIL')
                shutil.copyfile(SOURCE, artifact / SOURCE.name)
                record.update(errors=errors, artifact=str(artifact), replay_argv=replay + ['--case', cell.id])
                (artifact / 'fault.json').write_text(json.dumps(record, indent=2, sort_keys=True) + '\n')
            records.append(record)
            print(f'{record["status"]} {cell.id}' + (': ' + ', '.join(errors) if errors else ''), flush=True)
    report = {'result': 'FAIL' if failures else 'PASS', 'passed': len(cells) - failures,
              'failed': failures, 'executed': len(cells), 'identities': identities,
              'helper': helper, 'cases': records, 'replay_argv': replay,
              'limitations': ['trusted fixed inputs, not a security sandbox',
                              'Linux/glibc dynamic interposition; timestamps not compared']}
    reports = artifacts / 'campaigns'
    reports.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', prefix='stdio-faults-',
                                     dir=reports, delete=False) as output:
        json.dump(report, output, indent=2, sort_keys=True)
        output.write('\n')
        print(f'Fault campaign report: {output.name}')
    print(f'Filesystem faults: passed={len(cells) - failures}, failed={failures}')
    return int(bool(failures))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--self-test', action='store_true')
    mode.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle', type=Path, default=conformance.ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--oracle-profile', type=Path, default=oracle.PROFILE)
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--artifacts', type=Path, default=conformance.ROOT / 'artifacts/stdio-faults')
    args = parser.parse_args(argv)
    try:
        ids = {cell.id for cell in inventory()}
        if set(args.case) - ids:
            raise conformance.HarnessError(f'unknown fault case IDs: {sorted(set(args.case) - ids)}')
        cells = tuple(cell for cell in inventory() if not args.case or cell.id in args.case)
        profile = oracle.verify(args.oracle.expanduser().resolve(), args.oracle_profile)
        reference = conformance.executable_identity(args.oracle, oracle=True)
        reference['profile'] = profile
        candidate = reference if args.self_test else conformance.executable_identity(args.candidate)
        replay = [sys.executable, str(Path(__file__).resolve()), '--oracle', reference['path'],
                  '--oracle-profile', str(args.oracle_profile.expanduser().resolve()),
                  *(['--self-test'] if args.self_test else ['--candidate', candidate['path']])]
        return run_campaign(cells, Path(reference['path']), Path(candidate['path']),
                            args.artifacts.expanduser().resolve(),
                            {'oracle': reference, 'candidate': candidate}, replay)
    except (conformance.HarnessError, oracle.OracleError, OSError) as error:
        print(f'stdio-faults: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
