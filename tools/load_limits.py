#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original controlled Linux load campaign with independently anchored results."""
from __future__ import annotations
import argparse
import dataclasses
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import conformance as harness
import oracle

ROOT = Path(__file__).resolve().parents[1]
SOURCE = Path(__file__).with_suffix('.c')
HELPER = b'''import fcntl
from pathlib import Path
import sys
import time
name, mode = sys.argv[1:]
def wait(predicate):
    end = time.monotonic() + 3
    while not predicate():
        if time.monotonic() > end:
            raise RuntimeError('bounded load barrier failed')
        time.sleep(.002)
with open('lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    active = int(Path('active').read_text()) + 1
    Path('active').write_text(str(active))
    Path('maximum').write_text(str(max(active, int(Path('maximum').read_text()))))
    Path(name + '.started').write_text('started\\n')
if mode == 'serial' or mode.startswith('fail'):
    if name == 'a':
        wait(lambda: Path('expanded-c').exists())
elif mode == 'dynamic' and name == 'a':
    wait(lambda: Path('expanded-c').exists())
    wait(lambda: all(Path(n + '.started').exists() for n in 'abc'))
else:
    wait(lambda: all(Path(n + '.started').exists() for n in 'abc'))
with open('lock', 'a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    Path('active').write_text(str(int(Path('active').read_text()) - 1))
    with open('order', 'a') as out:
        out.write(name + '\\n')
if name == 'a' and mode.startswith('fail'):
    raise SystemExit(7)
'''

@dataclasses.dataclass(frozen=True)
class Cell:
    id: str
    proc: bytes
    limit: str
    behavior: str
    mode: str = 'normal'
    average: str = '100'
    trace: bytes = b'proc\n'
    stderr: bytes = b''

    @property
    def maximum(self):
        return 1 if self.behavior == 'serial' or self.behavior.startswith('fail') else 3

    @property
    def makefile(self):
        lines = ['.PHONY: all a b c', 'all: a b c',
                 "\t@printf 'active=%s maximum=%s\\n' \"$$(cat active)\" \"$$(cat maximum)\""]
        for name in 'abc':
            lines += [name + ':', '\t@$(file >expanded-' + name + ')python3 helper.py ' + name + ' ' + self.behavior]
        if self.behavior == 'dynamic':
            lines[1] = 'all: a b c d'
            lines += ['d:', '\t@$(file >load.input,0 0 0 0/100 1)true']
        return ('\n'.join(lines) + '\n').encode()


def inventory():
    cells = []
    for name, count, limit, behavior in [
        ('above', '3', '2', 'serial'), ('equal', '2', '2', 'parallel'),
        ('below', '1', '2', 'parallel'), ('zero', '0', '0', 'parallel'),
        ('fraction', '1', '.5', 'serial'), ('unsigned-wrap', '4294967296', '0', 'parallel'),
        ('unsigned-overflow', '18446744073709551616', '4000000000', 'serial'),
        ('disabled', '999', '-2', 'parallel'),
    ]:
        cells.append(Cell(name, f'99 88 77 {count}/100 1\n'.encode(), limit, behavior,
                          trace=b'' if name == 'disabled' else b'proc\n'))
    cells.append(Cell('load-drops-after-peer-completion', b'0 0 0 99/100 1\n', '2', 'dynamic'))
    cells.append(Cell('interrupted-read', b'0 0 0 99/100 1\n', '2', 'serial',
                      'read-eintr', trace=b'proc\nread-fault\n'))
    for name, proc, mode, average, limit, behavior, trace in [
        ('open-fallback', b'', 'open-denied', '100', '2', 'serial', b'proc\naverage\n'),
        ('malformed-fallback', b'not a load record\n', 'normal', '100', '2', 'serial', b'proc\naverage\n'),
        ('read-fallback', b'0 0 0 2/100 1\n', 'read-eio', '100', '2', 'serial', b'proc\nread-fault\naverage\n'),
        ('low-average-fallback', b'', 'open-denied', '0', '100', 'parallel', b'proc\naverage\n'),
        ('equal-average-fallback', b'', 'open-denied', '0', '0', 'serial', b'proc\naverage\n'),
    ]:
        cells.append(Cell(name, proc, limit, behavior, mode, average, trace))
    for kind, diagnostic in [('eio', b'make: cannot enforce load limit: getloadavg: Input/output error\n'),
                             ('unsupported', b'make: cannot enforce load limits on this operating system\n')]:
        cells.append(Cell('average-' + kind, b'', '0', 'serial', 'open-denied', kind,
                          b'proc\naverage\n', diagnostic))
    for mode, suffix in [('fail', b''), ('fail-keep', b"make: Target 'all' not remade because of errors.\n"), ('fail-ignore', b'')]:
        cells.append(Cell('queued-' + mode, b'0 0 0 99/100 1\n', '2', mode,
                          stderr=(b'make: [Makefile:5: a] Error 7 (ignored)\n' if mode == 'fail-ignore' else b'make: *** [Makefile:5: a] Error 7\n') + suffix))
    return tuple(cells)


def materialize(cell, path):
    path.mkdir()
    (path / 'Makefile').write_bytes(cell.makefile)
    (path / 'helper.py').write_bytes(HELPER)
    (path / 'load.input').write_bytes(cell.proc)
    for name in ('active', 'maximum'):
        (path / name).write_text('0')


def execute(cell, root, fixture, binary, helper):
    trace = root / 'load.trace'
    trace.unlink(missing_ok=True)
    case = harness.Case(fixture, cell.id, 'runtime.load', 'required',
                        ('-rR', '--no-print-directory', '-j4', '-l' + cell.limit,
                         *(['-k'] if cell.behavior == 'fail-keep' else ['-i'] if cell.behavior == 'fail-ignore' else [])), 6, {}, {})
    harness.reset_workspace(case, root, binary)
    environment = harness.clean_environment(root, {
        'LD_PRELOAD': str(helper), 'RMAKE_TEST_LOAD': cell.mode,
        'RMAKE_LOAD_AVERAGE': cell.average, 'RMAKE_LOAD_TRACE': str(trace)})
    out, err, status, limit, leftovers = harness.capture(
        [str(root / 'bin/make'), *case.args], root / 'work', environment, case.timeout)
    # Parallel completion order has no specified order. The fixture checks
    # every target exactly once; serial queue order remains a strict anchor.
    order = (root / 'work/order').read_bytes() if (root / 'work/order').exists() else b''
    files = harness.snapshot({name: root / name for name in ('work', 'home', 'tmp')})
    # Only the explicitly unordered completion journal uses a named membership
    # validator. Every other byte/file observation remains exact.
    files.pop('work/order', None)
    observation = harness.Observation(out, err, status, limit, files, leftovers)
    actual_trace = trace.read_bytes() if trace.exists() else b''
    return observation, anchors(cell, observation, order, actual_trace), order, actual_trace


def anchors(cell, observation, order, actual_trace):
    errors = []
    if observation.limit or observation.leftover_processes:
        errors.append("process containment")
    if cell.behavior == 'serial' or cell.behavior.startswith('fail'):
        expected_order = b'a\n' if cell.behavior == 'fail' else b'a\nc\nb\n'
        if order != expected_order: errors.append('serial LIFO queue order')
    elif sorted(order.splitlines()) != [b'a', b'b', b'c']:
        errors.append('all parallel targets exactly once')
    failed = cell.behavior in ('fail', 'fail-keep')
    expected_output = b'' if failed else f'active=0 maximum={cell.maximum}\n'.encode()
    if (observation.stdout, observation.stderr, observation.returncode) != (expected_output, cell.stderr, 2 if failed else 0):
        errors.append('independent status/output/concurrency anchors')
    if actual_trace != cell.trace: errors.append('load observation mechanism')
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--repeat', type=int, default=3)
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--artifacts', type=Path, default=ROOT / 'artifacts/load')
    args = parser.parse_args()
    if bool(args.candidate) == args.self_test: parser.error('choose candidate or self-test')
    if not 1 <= args.repeat <= 100: parser.error('repeat must be 1-100')
    profile = oracle.verify(args.oracle.resolve())
    candidate = args.oracle.resolve() if args.self_test else args.candidate.resolve()
    identity = harness.executable_identity(candidate)
    args.artifacts.mkdir(parents=True, exist_ok=True)
    destination = Path(tempfile.mkdtemp(prefix='load-', dir=args.artifacts.resolve()))
    helper = destination / 'load.so'
    subprocess.run(['cc', '-shared', '-fPIC', '-O2', '-Wall', '-Wextra', '-Werror',
                    str(SOURCE), '-ldl', '-o', str(helper)], check=True)
    cells = inventory()
    if set(args.case) - {c.id for c in cells}: parser.error('unknown case')
    results = []
    for cell in cells:
        if args.case and cell.id not in args.case: continue
        fixture = destination / ('input-' + cell.id)
        materialize(cell, fixture)
        for repeat in range(args.repeat):
            root = destination / 'run'
            root.mkdir(exist_ok=True)
            baseline, baseline_errors, baseline_order, baseline_trace = execute(cell, root, fixture, args.oracle.resolve(), helper)
            observed, errors, order, trace = execute(cell, root, fixture, candidate, helper)
            errors += baseline_errors + harness.differences(baseline, observed)
            row = {'case': cell.id, 'repeat': repeat, 'errors': errors,
                   'oracle': dataclasses.asdict(baseline), 'candidate': dataclasses.asdict(observed),
                   'oracle_order': baseline_order, 'candidate_order': order,
                   'oracle_trace': baseline_trace, 'candidate_trace': trace}
            results.append(row)
            print('FAIL' if errors else 'PASS', cell.id, repeat, errors, flush=True)
    report = {'oracle_profile': profile, 'candidate_identity': identity,
              'source_sha256': harness.sha256_file(SOURCE), 'runner_sha256': harness.sha256_file(Path(__file__)),
              'results': results,
              'limitations': ['Trusted fixtures and LD_PRELOAD, not a security sandbox.',
                              'Explicitly unordered parallel completion journal has an exact membership validator; raw order retained.']}
    (destination / 'report.json').write_text(json.dumps(report, default=lambda x: {'hex': x.hex()} if isinstance(x, bytes) else str(x), indent=2) + '\n')
    print('Report:', destination / 'report.json')
    return int(any(r['errors'] for r in results))

if __name__ == '__main__':
    raise SystemExit(main())
