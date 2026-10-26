#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original anchored archive metadata/order probes on the pinned Linux profile.

Trusted Makefiles and a narrow syscall interposer, not a security sandbox.
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

ROOT = Path(__file__).resolve().parents[1]
PROTECTED = b'Original archive observation campaign anchor.\n'


@dataclasses.dataclass(frozen=True)
class Cell:
    id: str
    role: str
    bad: bool
    newer: bool
    links: bool
    variant: str
    operation: str
    kind: str
    mode: str
    intermediate: bool = False
    hidden: bool = False
    parallel: bool = False

    @property
    def fault(self):
        if self.operation in ('stat', 'post-stat'):
            return 'symlink-stat-' + self.kind
        return 'archive-' + self.operation.removeprefix('touch-').removeprefix('query-') + '-' + self.kind


def inventory():
    cells = []
    for variant in ('local', 'vpath', 'gpath', 'declared'):
        for role in ('member', 'container'):
            for bad in (False, True):
                for newer in (False, True):
                    for links in (False, True):
                        for kind in ('eintr', 'eio', 'eacces', 'enoent', 'enotdir'):
                            for mode in ('normal', 'dry', 'query', 'touch'):
                                name = '-'.join((variant, role, 'bad' if bad else 'valid',
                                                 'new' if newer else 'old', 'L' if links else 'default',
                                                 'stat', kind, mode))
                                cells.append(Cell(name, role, bad, newer, links, variant, 'stat', kind, mode))
        for bad in (False, True):
            for links in (False, True):
                for mode in ('normal', 'dry', 'query', 'touch'):
                    name = '-'.join((variant, 'container', 'bad' if bad else 'valid',
                                     'L' if links else 'default', 'open', 'eintr', mode))
                    cells.append(Cell(name, 'container', bad, False, links, variant, 'open', 'eintr', mode))
        for operation in ('read-signature', 'read-header', 'read-next', 'seek'):
            for kind in (('eintr', 'eio') if operation == 'seek' else ('eintr', 'eio', 'eof', 'short')):
                for bad in (False, True):
                    for links in (False, True):
                        for mode in ('normal', 'dry', 'query', 'touch'):
                            name = '-'.join((variant, 'container', 'bad' if bad else 'valid',
                                             'L' if links else 'default', operation, kind, mode))
                            cells.append(Cell(name, 'container', bad, False, links, variant, operation, kind, mode))
    for operation in ('touch-read-signature', 'touch-read-header', 'touch-read-next', 'touch-seek',
                      'query-read-signature', 'query-read-header', 'query-read-next', 'query-seek'):
        for kind in (('eintr', 'eio') if operation.endswith('seek') else ('eintr', 'eio', 'eof', 'short')):
            for links in (False, True):
                name = '-'.join(('local', 'container', 'L' if links else 'default', operation, kind))
                cells.append(Cell(name, 'container', False, False, links, 'local', operation, kind,
                                  'query' if operation.startswith('query-') else 'touch'))
    for role in ('member', 'container'):
        for kind in ('eio', 'eacces'):
            for links in (False, True):
                for mode in ('normal', 'dry', 'query', 'touch'):
                    for hidden in (False, True):
                        name = '-'.join(('local', role, 'intermediate', 'hidden' if hidden else 'visible',
                                         'L' if links else 'default', kind, mode))
                        cells.append(Cell(name, role, False, role == 'member', links, 'local',
                                          'stat', kind, mode, True, hidden))
    for variant in ('local', 'vpath', 'gpath', 'declared'):
        for role in ('member', 'container'):
            for bad in (False, True):
                for newer in ((False, True) if role == 'member' else (False,)):
                    for kind in ('eintr', 'eio', 'eacces', 'enoent', 'enotdir'):
                        for parallel in (False, True):
                            name = '-'.join((variant, role, 'post', 'bad' if bad else 'valid',
                                             'new' if newer else 'old', kind, 'parallel' if parallel else 'serial'))
                            cells.append(Cell(name, role, bad, newer, False, variant,
                                              'post-stat', kind, 'normal', parallel=parallel))
    return tuple(cells)


def names(cell):
    archive, member = ('archive.a', 'effect-probe') if cell.role == 'member' else ('effect-probe', 'member')
    physical = archive if cell.variant == 'local' else 'd/' + archive
    return archive, member, physical


def archive_bytes(member, bad, sequence=False):
    # An original one-member archive with fixed, deliberately unambiguous fields.
    values = [member.encode() + b'/', b'bad' if bad else b'1600000020', b'0', b'0', b'100644', b'8']
    header = b''.join(value.ljust(width, b' ') for value, width in zip(values, (16, 12, 6, 6, 8, 10)))
    prefix = archive_bytes('ignored', False)[8:] if sequence else b''
    return b'!<arch>\n' + prefix + header + b'`\n' + b'payload\n'


def inputs(cell):
    archive, member, physical = names(cell)
    prefix = b'.DEFAULT_GOAL=out\n'
    if cell.variant != 'local':
        prefix += b'VPATH=d\n'
    if cell.variant == 'gpath':
        prefix += b'GPATH=d\n'
    if cell.variant == 'declared':
        prefix += f'{physical}({member}):X=declared\n'.encode()
    if cell.intermediate:
        prefix += f'.INTERMEDIATE: {archive}({member})\n'.encode()
    if cell.hidden:
        prefix += f'out:.EXTRA_PREREQS={archive}({member})\n'.encode()
    dependency = '' if cell.hidden else f'{archive}({member})'
    source = prefix + (f'$(shell python3 setup.py)\nout: {dependency}\n'
                       "\t@printf 'build=%s\\n' '$?'\n").encode()
    setup = ("import os\n"
             f"for name,seconds in [('out',10),({physical!r},0),({member!r},{30 if cell.newer else 0})]:\n"
             " os.utime(name,ns=((1600000000+seconds)*10**9,)*2)\n").encode()
    result = {'Makefile': faults.regular_file(source), 'setup.py': faults.regular_file(setup),
              physical: faults.regular_file(archive_bytes(member, cell.bad, cell.operation.endswith('read-next'))),
              member: faults.regular_file(b'original member\n'),
              'out': faults.regular_file(b'old output\n'),
              'protected.txt': faults.regular_file(PROTECTED)}
    if cell.variant != 'local':
        result['d'] = {'type': 'directory', 'mode': 0o755}
    if cell.operation == 'post-stat':
        setup = setup.replace(b"('out',10)", b"('out',40)")
        setup = setup.replace(f"({member!r},{30 if cell.newer else 0})".encode(), f"({member!r},0)".encode())
        source = prefix + (f'$(shell python3 setup.py)\nout: {dependency}\n'
                           "\t@printf 'build=%s\\n' '$?'\n"
                           '.PHONY: force\nforce:;\n'
                           f'{archive}({member}): force;@python3 post.py\n').encode()
        post = "from pathlib import Path\nimport os\n"
        if cell.bad:
            post += f"Path({physical!r}).write_bytes(Path('invalid.a').read_bytes())\n"
        if cell.newer:
            post += f"os.utime({member!r},ns=(1600000030000000000,)*2)\n"
        post += "Path('arm.marker').write_bytes(b'armed')\n"
        result.update({'Makefile': faults.regular_file(source), 'setup.py': faults.regular_file(setup),
                       'post.py': faults.regular_file(post.encode()),
                       physical: faults.regular_file(archive_bytes(member, False)),
                       'invalid.a': faults.regular_file(archive_bytes(member, True))})
    if cell.operation.startswith(('touch-', 'query-')):
        source += (f'{archive}({member}):\n'
                   "\t+@printf armed > arm.marker\n\t@printf forbidden\n"
                   ".PHONY: verify\nverify:\n\t+@RMAKE_STDIO_FAULT=none python3 normalize.py\n").encode()
        result['Makefile'] = faults.regular_file(source)
        start = 92 if cell.operation.endswith('read-next') else 24
        original = archive_bytes(member, False, cell.operation.endswith('read-next'))
        normalize = ("from pathlib import Path\nimport os\n"
                     f"p=Path({physical!r}); data=p.read_bytes(); original=bytes.fromhex({original.hex()!r})\n"
                     f"start={start}; end=start+12\n"
                     "changed=int(data[start:end] != original[start:end])\n"
                     "fixed=data[:start]+original[start:end]+data[end:]\n"
                     "assert fixed==original, 'non-date archive bytes changed'\n"
                     "print('changed='+str(changed))\n"
                     "p.write_bytes(fixed); os.utime(p,ns=(1600000000000000000,)*2)\n").encode()
        result['normalize.py'] = faults.regular_file(normalize)
    return result


def expected(cell):
    archive, member, physical = names(cell)
    diagnostic = b''
    if cell.operation == 'stat' and cell.kind in ('eio', 'eacces'):
        diagnostic = b'make: stat: effect-probe: ' + {
            'eio': b'Input/output error', 'eacces': b'Permission denied'}[cell.kind] + b'\n'
    lost_header = (cell.operation.startswith('read-') and cell.kind in ('eio', 'eof')) or (cell.operation == 'seek' and cell.kind == 'eio')
    header_error = cell.bad and cell.operation != 'open' and not lost_header
    absent = cell.operation == 'open' or lost_header or (cell.newer and (cell.role == 'container' or cell.kind == 'eintr'))
    if header_error:
        stdout, status = b'', 2
        diagnostic += f'make: *** Invalid date for archive {physical} member {member}.  Stop.\n'.encode()
    elif absent:
        target = physical if cell.variant in ('gpath', 'declared') else archive
        stdout, status = b'', 2
        diagnostic += f"make: *** No rule to make target '{target}({member})', needed by 'out'.  Stop.\n".encode()
    else:
        visible_member = '' if cell.hidden else member
        stdout = {'normal': f'build={visible_member}\n'.encode(),
                  'dry': f"printf 'build=%s\\n' '{visible_member}'\n".encode(),
                  'query': b'', 'touch': b'touch out\n'}[cell.mode]
        status = 1 if cell.mode == 'query' else 0
    if cell.operation.startswith(('touch-', 'query-')):
        failure = cell.kind in ('eio', 'eof')
        status = 2 if failure else 0
        stdout = ((f'touch {archive}({member})\n' * (1 if failure else 2))
                  + f'changed={0 if failure else 1}\n').encode()
        diagnostic = b''
        if failure:
            if cell.kind == 'eof' and not cell.operation.endswith('read-signature'):
                diagnostic = f"make: touch: Member '{member}' does not exist in '{archive}'\n".encode()
            else:
                diagnostic = f"make: touch: '{archive}' is not a valid archive\n".encode()
        if failure:
            diagnostic += f"make: *** Archive member '{archive}({member})' may be bogus; not deleted\n".encode()
        if cell.operation.startswith('query-'):
            stdout, status = b'changed=0\n', 1
            diagnostic = f"make: *** Archive member '{archive}({member})' may be bogus; not deleted\n".encode()
    if cell.operation == 'post-stat':
        diagnostic = b''
        if cell.kind in ('eio', 'eacces'):
            selected = physical if cell.role == 'container' and cell.variant == 'gpath' else 'effect-probe'
            diagnostic = f'make: stat: {selected}: '.encode() + {
                'eio': b'Input/output error', 'eacces': b'Permission denied'}[cell.kind] + b'\n'
        stdout, status = b'', 0
        if cell.bad:
            diagnostic += f'make: *** Invalid date for archive {physical} member {member}.  Stop.\n'.encode()
            status = 2
        elif cell.role == 'member' and cell.newer and cell.kind == 'eintr':
            stdout = f'build={member}\n'.encode()
        elif cell.role == 'container' and cell.kind != 'eintr' and not cell.parallel and cell.variant in ('local', 'gpath'):
            stdout = b'build=\n'
    filesystem = {name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp', 'work')}
    filesystem.update({'work/' + name: entry for name, entry in inputs(cell).items()})
    trace = (b'interrupted\n' if cell.operation == 'open' else b'symlink-fault\n' if cell.operation in ('stat', 'post-stat')
             else b'archive-seek-fault\n' if cell.operation.endswith('seek') else b'archive-read-fault\n')
    filesystem['work/fault.log'] = faults.regular_file(trace)
    if cell.operation == 'post-stat':
        filesystem['work/arm.marker'] = faults.regular_file(b'armed')
        if cell.bad:
            filesystem['work/' + physical] = faults.regular_file(archive_bytes(member, True))
    if cell.operation.startswith(('touch-', 'query-')):
        filesystem['work/arm.marker'] = faults.regular_file(b'armed')
    return harness.Observation(stdout, diagnostic, status, None, filesystem)


def materialize(cell, parent, library):
    path = parent / cell.id
    path.mkdir(parents=True)
    for name, entry in inputs(cell).items():
        target = path / name
        if entry['type'] == 'directory':
            target.mkdir(exist_ok=True)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(base64.b64decode(entry['content']))
            target.chmod(0o644)
    flags = ('-rR', '--no-print-directory') + (('-L',) if cell.links else ())
    flags += {'normal': (), 'dry': ('-n',), 'query': ('-q',), 'touch': ('-t',)}[cell.mode]
    environment = {'LD_PRELOAD': str(library), 'RMAKE_STDIO_FAULT': cell.fault}
    if cell.operation == 'post-stat':
        environment['RMAKE_ARCHIVE_ARMED'] = '1'
        if cell.parallel:
            flags += ('-j2',)
    if cell.operation.startswith(('touch-', 'query-')):
        archive, member, _ = names(cell)
        flags += ('-B', '-k', f'{archive}({member})', 'verify')
        environment['RMAKE_ARCHIVE_ARMED'] = '1'
    return harness.Case(path, cell.id, 'archives.observation-faults', 'required', flags, 5, environment, {})


def run(cells, reference, candidate, artifacts, identities, replay):
    records = []
    with tempfile.TemporaryDirectory(prefix='rmake-archive-times-') as temporary:
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
            record = {'case': cell.id, 'cell': dataclasses.asdict(cell), 'fault': cell.fault,
                      'status': 'FAIL' if errors else 'PASS', 'expected': harness.observation_json(anchor),
                      'expected_stdout_hex': anchor.stdout.hex(), 'expected_stderr_hex': anchor.stderr.hex()}
            if errors:
                artifact = harness.save_artifact(artifacts, case, workspace, identities,
                                                 observed['oracle'], observed['candidate'], errors, 'FAIL')
                shutil.copyfile(faults.SOURCE, artifact / faults.SOURCE.name)
                record.update(errors=errors, artifact=str(artifact), replay_argv=replay + ['--case', cell.id])
                (artifact / 'archive-times.json').write_text(json.dumps(record, indent=2, sort_keys=True) + '\n')
            records.append(record)
            print(record['status'] + ' ' + cell.id + (': ' + ', '.join(errors) if errors else ''), flush=True)
    failures = sum(record['status'] == 'FAIL' for record in records)
    report = {'result': 'FAIL' if failures else 'PASS', 'passed': len(cells) - failures,
              'failed': failures, 'executed': len(cells), 'identities': identities,
              'helper': helper, 'cases': records, 'replay_argv': replay,
              'limits': 'trusted reviewed Makefiles; no security sandbox; fixed input mtimes; boundary bytes/status/files only'}
    campaigns = artifacts / 'campaigns'
    campaigns.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', prefix='archive-', dir=campaigns, delete=False) as file:
        json.dump(report, file, indent=2, sort_keys=True)
        file.write('\n')
        report_path = file.name
    print(f'Archive timestamps: pass={len(cells) - failures}, fail={failures}')
    print(f'Campaign report: {report_path}')
    return 1 if failures else 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--self-test', action='store_true')
    mode.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--oracle-profile', type=Path, default=oracle.PROFILE)
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--artifacts', type=Path, default=ROOT / 'artifacts/archive-times')
    args = parser.parse_args(argv)
    try:
        cells = inventory()
        if set(args.case) - {cell.id for cell in cells}:
            raise harness.HarnessError('unknown archive timestamp case')
        cells = tuple(cell for cell in cells if not args.case or cell.id in args.case)
        reference = harness.executable_identity(args.oracle, oracle=True)
        reference['profile'] = oracle.verify(Path(reference['path']), args.oracle_profile)
        candidate = reference if args.self_test else harness.executable_identity(args.candidate)
        replay = [sys.executable, str(Path(__file__).resolve()), '--oracle', reference['path'],
                  '--oracle-profile', str(args.oracle_profile.expanduser().resolve()),
                  *(['--self-test'] if args.self_test else ['--candidate', candidate['path']])]
        return run(cells, Path(reference['path']), Path(candidate['path']), args.artifacts.expanduser().resolve(),
                   {'oracle': reference, 'candidate': candidate}, replay)
    except (harness.HarnessError, oracle.OracleError, OSError) as error:
        print(f'archive-times: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
