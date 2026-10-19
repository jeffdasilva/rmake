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
import os
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
ARCHIVE = (b'!<arch>\n' + b'a.o/'.ljust(16) + b'0'.ljust(12)
           + b'0'.ljust(6) * 2 + b'644'.ljust(8) + b'0'.ljust(10) + b'`\n')


@dataclasses.dataclass(frozen=True)
class Cell:
    id: str
    fault: str
    operation: bytes
    stdout: bytes
    stage: str | None
    final: bytes
    trace: bytes
    initial: bytes = INITIAL

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
    for mode, first, later, before, cached, literal, fresh in (
        ('files', b'*.t', b'*.t', b'a.t', b'a.t', b'b.t', b'b.t'),
        ('files-escaped-dot', b'a\\.t', b'a\\.t', b'a.t', b'a.t', b'b.t', b''),
        ('files-escaped-letter', b'\\a.t', b'\\a.t', b'a.t', b'a.t', b'b.t', b''),
        ('missing', b'missing/*.t', b'missing/*.t', b'', b'', b'missing/b.t', b'missing/b.t'),
        ('alias', b'd/*.t', b'link/*.t', b'd/a.t', b'link/a.t', b'link/b.t', b'link/b.t'),
        ('replaced', b'd/*.t', b'd/*.t', b'd/a.t', b'd/a.t', b'd/b.t', b'd/b.t'),
    ):
        expression = (b'$(info before=<$(wildcard ' + first + b')>)\n'
                      b'$(info data=<$(file <effect-probe)>)\n'
                      b'$(info cached=<$(wildcard ' + later + b')>)\n'
                      b'$(info literal=<$(wildcard ' + literal + b')>)\n'
                      b'$(file >invalidate)\n$(info fresh=<$(wildcard ' + later + b')>)')
        stdout = (b'begin\nbefore=<' + before + b'>\ndata=<original-content>\ncached=<'
                  + cached + b'>\nliteral=<' + literal + b'>\nfresh=<' + fresh + b'>\n')
        fault = 'wildcard-files' if mode.startswith('files-') else 'wildcard-' + mode
        cells.append(Cell('wildcard-' + mode, fault, expression, stdout,
                          None, INITIAL, b'external-mutation\n'))
    cells.append(Cell('wildcard-stat-eintr', 'stat-eintr',
                      b'$(info path=<$(wildcard effect-probe)>)',
                      b'begin\npath=<>\n', None, INITIAL, b'interrupted\n'))
    cells.append(Cell('archive-open-eintr', 'archive-open-eintr',
                      b'$(info path=<$(wildcard effect-probe(*))>)',
                      b'begin\npath=<effect-probe(*)>\n', None, ARCHIVE,
                      b'interrupted\n', initial=ARCHIVE))
    return tuple(cells)


def wildcard_entries(cell: Cell, final: bool) -> dict:
    if not cell.fault.startswith('wildcard-'):
        return {}
    if cell.fault == 'wildcard-files':
        entries = {'b.t' if final else 'a.t': regular_file(b'new\n' if final else b'old\n')}
    elif cell.fault == 'wildcard-missing':
        entries = {'missing': {'type': 'directory', 'mode': 0o755},
                   'missing/b.t': regular_file(b'new\n')} if final else {}
    else:
        entries = {'d': {'type': 'directory', 'mode': 0o755},
                   'd/b.t' if final else 'd/a.t': regular_file(b'new\n' if final else b'old\n')}
        if cell.fault == 'wildcard-alias':
            entries['link'] = {'type': 'symlink', 'mode': 0o777, 'target': 'd'}
        elif final:
            entries.update({'old': {'type': 'directory', 'mode': 0o755},
                            'old/a.t': regular_file(b'old\n')})
    if final:
        entries['invalidate'] = regular_file(b'')
    return entries


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
    filesystem.update({'work/' + name: item for name, item in wildcard_entries(cell, True).items()})
    stderr = (f'Makefile:3: *** {cell.stage}: effect-probe: Input/output error.  Stop.\n'.encode()
              if cell.stage else b'')
    return conformance.Observation(cell.stdout.replace(b'@WORK@', bytes(workspace / 'work')),
                                   stderr, 2 if cell.stage else 0, None, filesystem)


def materialize(cell: Cell, parent: Path, library: Path) -> conformance.Case:
    path = parent / cell.id
    path.mkdir(parents=True)
    for name, contents in {'Makefile': cell.makefile, 'effect-probe': cell.initial,
                           'protected.txt': PROTECTED}.items():
        (path / name).write_bytes(contents)
        (path / name).chmod(0o644)
    for name, entry in wildcard_entries(cell, False).items():
        target = path / name
        if entry['type'] == 'directory':
            target.mkdir()
        elif entry['type'] == 'symlink':
            os.symlink(entry['target'], target)
        else:
            target.write_bytes(base64.b64decode(entry['content']))
            target.chmod(0o644)
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
