#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original directory-search/cache probes with authored output and file anchors.

Trusted Makefiles and a bounded Linux/glibc test interposer, not a security
sandbox. The interposer mutates files, faults I/O and journals real stream
state; it never supplies directory entries or Make decisions.
"""
from __future__ import annotations

import argparse
import base64
import copy
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
ARGS = ('-rR', '--no-print-directory')
PROTECTED = b'Original directory-search campaign anchor.\n'


@dataclasses.dataclass(frozen=True)
class Cell:
    id: str
    source: bytes
    fault: str
    initial: dict
    final: dict
    stdout: bytes
    trace: bytes | None
    stderr: bytes = b''
    status: int = 0
    flags: tuple[str, ...] = ()


def inventory() -> tuple[Cell, ...]:
    cells = []
    for mutation, parent, warm in (
        ('alias', 'link', 'd'), ('replaced', 'd', 'd'), ('missing', 'missing', 'missing'),
    ):
        template = next(cell for cell in faults.inventory() if cell.id == 'wildcard-' + mutation)
        initial = faults.wildcard_entries(template, False)
        final = faults.wildcard_entries(template, True)
        del final['invalidate']
        before = b'' if mutation == 'missing' else b'd/a.t'
        for kind in ('vpath', 'gpath', 'implicit'):
            for name in ('a', 'b'):
                for refresh in (False, True):
                    for mode in ('normal', 'dry', 'query'):
                        target = (parent + '/' + name + '.out') if kind == 'implicit' else name + '.t'
                        lines = []
                        if kind != 'implicit':
                            lines.append('VPATH=' + parent)
                        if kind == 'gpath':
                            lines.append('GPATH=' + parent)
                        lines += ['warm := $(wildcard ' + warm + '/*.t)', '$(info warm=$(warm))',
                                  'read := $(file <effect-probe)']
                        if refresh:
                            lines.append('$(file >invalidate)')
                        lines += ['all: ' + target, '\t@echo selected=$<']
                        if kind == 'implicit':
                            lines += ['%.out: %.t', '\t@echo implicit=$@:$<']
                        lines.append('.DEFAULT: ; @echo fallback=$@')
                        commands = []
                        selected = target
                        if kind == 'implicit':
                            viable = (name == 'b' and refresh) or (name == 'a' and not refresh and mutation != 'missing')
                            if viable:
                                dependency = parent + '/' + name + '.t'
                                if name == 'a':
                                    commands.append('echo fallback=' + dependency)
                                commands.append('echo implicit=' + target + ':' + dependency)
                            else:
                                commands.append('echo fallback=' + target)
                        elif name == 'b' and (refresh or (kind == 'gpath' and mode == 'normal')):
                            if not refresh:
                                commands.append('echo fallback=' + target)
                            selected = parent + '/b.t'
                        else:
                            commands.append('echo fallback=' + target)
                        commands.append('echo selected=' + selected)
                        stdout = b'warm=' + before + b'\n'
                        if mode != 'query':
                            stdout += ''.join((command if mode == 'dry' else command.removeprefix('echo ')) + '\n'
                                              for command in commands).encode()
                        files = dict(final)
                        if refresh:
                            files['invalidate'] = faults.regular_file(b'')
                        case_id = f'{kind}-{mutation}-{name}-' + ('fresh-' if refresh else 'cached-') + mode
                        cells.append(Cell(case_id, ('\n'.join(lines) + '\n').encode(), template.fault,
                                          initial, files, stdout, b'external-mutation\n',
                                          status=1 if mode == 'query' else 0,
                                          flags={'normal': (), 'dry': ('-n',), 'query': ('-q',)}[mode]))
        for kind in ('vpath', 'gpath'):
            for role in ('mentioned', 'target', 'rejected-mention'):
                lines = ['VPATH=' + parent]
                if kind == 'gpath':
                    lines.append('GPATH=' + parent)
                lines += ['warm := $(wildcard ' + warm + '/*.t)', '$(info warm=$(warm))',
                          'read := $(file <effect-probe)', 'all: b.t', '\t@echo selected=$<']
                if role != 'mentioned':
                    lines.append('b.t:')
                if role == 'target':
                    lines.append(parent + '/b.t:')
                else:
                    lines.append('unused: ' + parent + '/b.t')
                lines.append('.DEFAULT: ; @echo fallback=$@')
                stdout = b'warm=' + before + b'\n'
                if role == 'rejected-mention':
                    stdout += b'selected=b.t\n'
                else:
                    stdout += ('selected=' + parent + '/b.t\n').encode()
                cells.append(Cell(f'{kind}-{mutation}-record-{role}', ('\n'.join(lines) + '\n').encode(),
                                  template.fault, initial, final, stdout, b'external-mutation\n'))
    # Failed searches warm the same directory that a later wildcard reads.
    original = {'d': {'type': 'directory', 'mode': 0o755}, 'd/a.t': faults.regular_file(b'old\n')}
    mutated = {'d': {'type': 'directory', 'mode': 0o755}, 'd/b.t': faults.regular_file(b'new\n')}
    for kind in ('local', 'vpath'):
        for dry in (False, True):
            source = (b'.SECONDEXPANSION:\n' + (b'VPATH=d\n' if kind == 'vpath' else b'')
                      + b'$(info begin)\nall: item.out\n%.out: d/never.t ; @echo wrong\n'
                      b'%.out: $$(if $$(file <effect-probe),,)$$(info cached=$$(wildcard d/*.t))\n'
                      b'\t@echo chosen=$@\n')
            stdout = b'begin\ncached=d/a.t\n' + (b'echo chosen=item.out\n' if dry else b'chosen=item.out\n')
            cells.append(Cell('search-warms-' + kind + ('-dry' if dry else '-normal'), source,
                              'wildcard-alias', original, mutated, stdout, b'external-mutation\n',
                              flags=('-n',) if dry else ()))
    for phase in ('partial', 'complete'):
        if phase == 'partial':
            rules = (b'%.out: d/a.t $$(if $$(file <effect-probe),,)\n'
                     b'\t@echo chosen=$@\n')
        else:
            rules = (b'%.out: d/never.t ; @echo wrong\n'
                     b'%.out: $$(if $$(file <effect-probe),,)\n'
                     b'\t@echo chosen=$@\n')
        source = b'.SECONDEXPANSION:\n$(info begin)\nall: item.out\n' + rules
        cells.append(Cell('stream-' + phase, source, 'directory-' + phase, original, original,
                          b'begin\nchosen=item.out\n', (phase + '\n').encode()))
    descriptor_check = (b'import os\nfrom pathlib import Path\n'
                        b'root=Path.cwd()\nleaks=[]\n'
                        b'for fd in Path("/proc/self/fd").iterdir():\n'
                        b' try: target=os.readlink(fd)\n'
                        b' except FileNotFoundError: continue\n'
                        b' if target in [str(root / ("d"+str(i))) for i in range(10)]: leaks.append(target)\n'
                        b'assert not leaks, leaks\nprint("child-streams=0")\n')
    files = {'check.py': faults.regular_file(descriptor_check)}
    for index in range(10):
        files['d' + str(index)] = {'type': 'directory', 'mode': 0o755}
        files[f'd{index}/a.t'] = faults.regular_file(b'old\n')
    source = (b'.SECONDEXPANSION:\n$(info begin)\nall: item.out\n%.out: '
              + b' '.join(f'd{i}/a.t'.encode() for i in range(10))
              + b' $$(if $$(file <effect-probe),,)\n\t@python3 check.py\n')
    cells.append(Cell('stream-limit-and-exec', source, 'directory-limit', files, files,
                      b'begin\nchild-streams=0\n', b'open=9 last=complete\n'))
    query = b'$(info begin)\n$(info seen=$(wildcard d/*.t))\nall:;@:\n'
    for fault, stdout, trace, stderr in (
        ('open-eintr', b'begin\nseen=d/a.t\n', b'interrupted\n', b''),
        ('read-eintr', b'begin\nseen=d/a.t\n', b'interrupted\n', b''),
        ('open-eacces', b'begin\nseen=\n', b'open-denied\n', b''),
        ('close-eio', b'begin\nseen=d/a.t\n', b'closed-once\n', b''),
        ('read-eio', b'begin\n', b'read-fault\n', b'make: *** readdir d: Input/output error.  Stop.\n'),
        ('read-tail-eio', b'begin\n', b'read-fault\n', b'make: *** readdir d: Input/output error.  Stop.\n'),
    ):
        cells.append(Cell('io-' + fault, query, 'directory-' + fault, original, original,
                          stdout, trace, stderr, status=2 if stderr else 0))
    for kind, source in (
        ('vpath', b'VPATH=d\n$(info begin)\nall: b.t\n.DEFAULT:;@:\n'),
        ('implicit', b'$(info begin)\nall: item.out\n%.out: d/a.t ; @:\n'),
        ('archive', b'vpath %.a d\n$(info begin)\nall: lib.a(m.o)\n.DEFAULT:;@:\n'),
            ):
        cells.append(Cell('io-search-' + kind, source, 'directory-read-eio', original, original,
                          b'begin\n', b'read-fault\n',
                          b'make: *** readdir d: Input/output error.  Stop.\n', status=2))
    source = b'vpath peer d\n$(info begin)\nall: owner\nowner peer &: ; @:\n'
    cells.append(Cell('io-group-skips-unused-peer', source, 'directory-read-eio', original, original,
                      b'begin\n', None))
    # The asynchronous completion path must expose the same late GPATH name.
    for cell in tuple(cells):
        if cell.id.startswith('gpath-') and cell.id.endswith('-b-cached-normal'):
            cells.append(dataclasses.replace(cell, id=cell.id.removesuffix('-normal') + '-parallel', flags=('-j2',)))
    existing = {**original, 'owner': faults.regular_file(b'old\n')}
    source = (b'seed := $(shell touch -d @1600000000 owner)\n'
              b'vpath peer d\n$(info begin)\nall: owner\nowner peer &: ; @:\n')
    cells.append(Cell('io-group-existing-owner-reads-peer', source, 'directory-read-eio', existing, existing,
                      b'begin\n', b'read-fault\n',
                      b'make: *** readdir d: Input/output error.  Stop.\n', status=2))
    initial = {**original, 'owner.out': faults.regular_file(b'old\n'), 'input': faults.regular_file(b'original\n')}
    final = {**initial, 'owner.out': faults.regular_file(b'built\n'), 'owner.peer': faults.regular_file(b'built\n')}
    source = (b'seed := $(shell touch -d @1600000000 owner.out; touch -d @1600000002 input)\n'
              b'vpath %.peer d\n$(info begin)\nall: owner.out\n'
              b'%.out %.peer &: input\n\t@echo built >owner.out; echo built >owner.peer\n')
    cells.append(Cell('io-implicit-group-skips-initial-peer', source, 'directory-read-eio', initial, final,
                      b'begin\n', None))
    initial = {**original, 'owner': faults.regular_file(b'old\n'), 'peer': faults.regular_file(b'old\n'),
               'input': faults.regular_file(b'original\n')}
    final = {name: value for name, value in initial.items() if name != 'peer'}
    final['owner'] = faults.regular_file(b'built\n')
    source = (b'seed := $(shell touch -d @1600000000 owner peer; touch -d @1600000002 input)\n'
              b'vpath peer d\n$(info begin)\nall: owner peer\nowner peer &: input\n'
              b'\t@echo built >owner; rm -f peer\n')
    cells.append(Cell('io-group-completion-keeps-local-observation', source, 'directory-read-eio', initial, final,
                      b'begin\n', None))
    source = (b'$(info begin)\n$(info first=$(wildcard d/*))\n$(file >invalidate)\n'
              b'$(info second=$(wildcard d/*))\nall:;@:\n')
    cells.append(Cell('io-denied-directory-refreshes', source, 'directory-open-eacces', original,
                      {**original, 'invalidate': faults.regular_file(b'')},
                      b'begin\nfirst=\nsecond=\n', b'open-denied\nopen-denied\n'))
    for original_cell in tuple(cells):
        if not original_cell.id.startswith('gpath-') or not original_cell.id.endswith('-b-cached-normal'):
            continue
        for mixed in (False, True):
            source = original_cell.source.replace(b'.DEFAULT: ; @echo fallback=$@',
                       b'.DEFAULT:\n\t+@echo fallback=$@' + (b'\n\t@echo skipped' if mixed else b''))
            parent = 'link' if '-alias-' in original_cell.id else 'd' if '-replaced-' in original_cell.id else 'missing'
            prefix = original_cell.stdout.split(b'fallback=', 1)[0]
            for mode, flags in (('dry', ('-n',)), ('query', ('-q',))):
                output = prefix
                if mode == 'dry': output += b'echo fallback=b.t\n'
                output += b'fallback=b.t\n'
                if mode == 'dry':
                    if mixed: output += b'echo skipped\n'
                    output += ('echo selected=' + ('b.t' if mixed else parent + '/b.t') + '\n').encode()
                key = original_cell.id.removesuffix('-normal') + ('-mixed-' if mixed else '-forced-') + mode
                cells.append(dataclasses.replace(original_cell, id=key, source=source, stdout=output,
                                                  flags=flags, status=1 if mode == 'query' else 0))
    for missing_first in (True, False):
        peers = b'owner searched missing' if missing_first else b'owner missing searched'
        source = (b'seed := $(shell touch -d @1600000000 owner)\n'
                  b'vpath searched d\n$(info begin)\nall: owner\n' + peers + b' &: ; @:\n')
        initial = {**original, 'owner': faults.regular_file(b'old\n')}
        cells.append(Cell('io-group-' + ('missing-peer-stops-lookup' if missing_first else 'searched-peer-errors-first'),
                          source, 'directory-read-eio', initial, initial, b'begin\n',
                          None if missing_first else b'read-fault\n',
                          b'' if missing_first else b'make: *** readdir d: Input/output error.  Stop.\n',
                          status=0 if missing_first else 2))
    for cell in tuple(cells):
        if cell.id.startswith('gpath-') and ('-forced-' in cell.id or '-mixed-' in cell.id):
            source = b'PLUS=+\n' + cell.source.replace(b'+@echo', b'@$(PLUS)echo')
            cells.append(dataclasses.replace(cell, id=cell.id + '-expanded-prefix', source=source))
    return tuple(cells)


def materialize(cell: Cell, parent: Path, library: Path) -> harness.Case:
    path = parent / cell.id
    path.mkdir(parents=True)
    entries = {'Makefile': faults.regular_file(cell.source),
               'effect-probe': faults.regular_file(faults.INITIAL),
               'protected.txt': faults.regular_file(PROTECTED), **cell.initial}
    for name, entry in sorted(entries.items()):
        target = path / name
        if entry['type'] == 'directory':
            target.mkdir(parents=True, exist_ok=True)
            target.chmod(0o755)
        elif entry['type'] == 'symlink':
            target.symlink_to(entry['target'])
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(base64.b64decode(entry['content']))
            target.chmod(0o644)
    harness.validate_fixture(path)
    return harness.Case(path, cell.id, 'resolution.directory-search', 'required',
                        ARGS + cell.flags, 5,
                        {'LD_PRELOAD': str(library), 'RMAKE_STDIO_FAULT': cell.fault}, {})


def expected(cell: Cell) -> harness.Observation:
    filesystem = {name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp', 'work')}
    files = {'Makefile': faults.regular_file(cell.source),
             'effect-probe': faults.regular_file(faults.INITIAL),
             'protected.txt': faults.regular_file(PROTECTED),
             **cell.final}
    if cell.trace is not None:
        files['fault.log'] = faults.regular_file(cell.trace)
    filesystem.update({'work/' + name: value for name, value in copy.deepcopy(files).items()})
    return harness.Observation(cell.stdout, cell.stderr, cell.status, None, filesystem)


def run_campaign(cells, reference, candidate, artifacts, identities, replay):
    records = []
    with tempfile.TemporaryDirectory(prefix='rmake-directory-search-') as temporary:
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
                      'expected': harness.observation_json(anchor),
                      'stdout_hex': cell.stdout.hex(), 'stderr_hex': cell.stderr.hex()}
            if errors:
                artifact = harness.save_artifact(artifacts, case, workspace, identities,
                                                  observed['oracle'], observed['candidate'], errors, 'FAIL')
                shutil.copyfile(faults.SOURCE, artifact / faults.SOURCE.name)
                record.update(errors=errors, artifact=str(artifact), replay_argv=replay + ['--case', cell.id])
                (artifact / 'directory.json').write_text(json.dumps(record, indent=2, sort_keys=True) + '\n')
            records.append(record)
            print(f'{record["status"]} {cell.id}' + (': ' + ', '.join(errors) if errors else ''), flush=True)
    failures = sum(record['status'] == 'FAIL' for record in records)
    report = {'result': 'FAIL' if failures else 'PASS', 'passed': len(cells) - failures,
              'failed': failures, 'executed': len(cells), 'identities': identities,
              'helper': helper, 'cases': records, 'replay_argv': replay,
              'limits': 'trusted original Makefiles; no security sandbox; timestamps outside this gate'}
    campaigns = artifacts / 'campaigns'
    campaigns.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', prefix='directory-',
                                     dir=campaigns, delete=False) as file:
        json.dump(report, file, indent=2, sort_keys=True)
        file.write('\n')
        report_path = file.name
    print(f'Directory search: pass={len(cells) - failures}, fail={failures}')
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
    parser.add_argument('--artifacts', type=Path, default=ROOT / 'artifacts/directory-search')
    args = parser.parse_args(argv)
    try:
        cells = inventory()
        ids = {cell.id for cell in cells}
        if set(args.case) - ids:
            raise harness.HarnessError(f'unknown directory-search cases: {sorted(set(args.case) - ids)}')
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
        print(f'directory-search: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
