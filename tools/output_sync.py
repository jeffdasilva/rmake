#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Bounded output grouping and mixed GNU/rmake recursion, with exact anchors."""
from __future__ import annotations
import argparse
import dataclasses
import json
from pathlib import Path
import tempfile
import conformance as harness
import oracle

ROOT = Path(__file__).resolve().parents[1]
MODES = ('none', 'line', 'target', 'recurse')
PROTECTED = b'Original protected output mutex.\n'
# Helpers force A1 < B1 < B2 < A2 by observing owned helper exit, not elapsed
# sleep ordering. Polls are bounded; make must reap A1 before B1 can finish.
HELPER = b'''import os
from pathlib import Path
import sys,time
name=sys.argv[1]
def wait(test):
    end=time.monotonic()+3
    while not test():
        if time.monotonic()>end:raise RuntimeError('bounded output barrier')
        time.sleep(.002)
if name=='a1':
    Path('a.pid').write_text(str(os.getpid()))
    Path('a.start').touch()
    wait(lambda:Path('b.start').exists())
elif name=='b1':
    Path('b.start').touch()
    wait(lambda:Path('a.pid').exists() and Path('a.pid').read_text())
    pid=int(Path('a.pid').read_text())
    wait(lambda:not Path('/proc',str(pid)).exists())
    Path('a.pid').unlink()
    wait(lambda:Path('a.continue').exists())
elif name=='b2':
    Path('b.pid').write_text(str(os.getpid()))
    Path('b.done').touch()
else:
    Path('a.continue').touch()
    wait(lambda:Path('b.done').exists())
    pid=int(Path('b.pid').read_text())
    wait(lambda:not Path('/proc',str(pid)).exists())
    Path('b.pid').unlink()
suffix = ':' + 'x' * 32768 if os.environ.get('OUTPUT_LARGE') == '1' else ''
os.write(1,(name+'-out'+suffix+'\\n').encode())
os.write(2,(name+'-err'+suffix+'\\n').encode())
'''

@dataclasses.dataclass(frozen=True)
class Cell:
    mode: str
    merged: bool
    warnings: bool = False
    child: str = ''
    owned: bool = False
    stress: bool = False

    @property
    def id(self):
        return '-'.join((self.mode, 'merged' if self.merged else 'separate',
                         'recursive-' + self.child if self.child else 'config' if self.warnings else 'phase',
                         ('owned' if self.owned else 'borrowed') + ('-large' if self.stress else '')))

    @property
    def source(self):
        if self.child:
            return b'.PHONY: all a b\nall: a b\na:\n\t+@$(OUTPUT_CHILD) --no-print-directory -f child.mk BRANCH=a\nb:\n\t+@$(OUTPUT_CHILD) --no-print-directory -f child.mk BRANCH=b\n'
        return (('SHELL = $(warning config-$@)/bin/sh\n' if self.warnings else '') + '''$(info parse-before)
$(shell printf 'parse-err\\n' >&2)
$(info parse-after)
.PHONY: all a b
all: a b
a:
	@$(info expand-a1)python3 helper.py a1
	@$(info expand-a2)python3 helper.py a2
b:
	@$(info expand-b1)python3 helper.py b1
	@$(info expand-b2)python3 helper.py b2
''').encode()

    def expected(self):
        # Describe the four ordered helper events independently of either engine.
        def event(name):
            suffix = ':' + 'x' * 32768 if self.stress else ''
            return [(False, name + '-out' + suffix), (True, name + '-err' + suffix)]
        expansion = lambda name: [(False, 'expand-' + name + '1'), (False, 'expand-' + name + '2')]
        configuration = lambda name, number: [(True, f'Makefile:{8 if name == "a" else 11 if number == 1 else 12}: config-{name}')]
        if self.child:
            order = ('a1', 'b1', 'b2', 'a2') if self.mode in ('none', 'line') else ('b1', 'b2', 'a1', 'a2')
            events = [entry for name in order for entry in event(name)]
        else:
            events = [(False, 'parse-before')]
            if self.warnings: events += [(True, 'Makefile:3: config-')]
            events += [(True, 'parse-err'), (False, 'parse-after')]
            def first(name):
                result = expansion(name)
                if self.warnings: result += configuration(name, 1)
                return result
            # The second SHELL expansion has no selected child buffer.
            second_a = [(True, 'Makefile:9: config-a')] if self.warnings else []
            second_b = [(True, 'Makefile:12: config-b')] if self.warnings else []
            if self.mode == 'none':
                events += first('a') + first('b') + event('a1') + second_a + event('b1') + second_b + event('b2') + event('a2')
            elif self.mode == 'line':
                events += first('a') + event('a1') + second_a + first('b') + event('b1') + second_b + event('b2') + event('a2')
            else:
                events += second_a + second_b + first('b') + event('b1') + event('b2') + first('a') + event('a1') + event('a2')
        out = b''.join((message + '\n').encode() for error, message in events if self.merged or not error)
        err = b'' if self.merged else b''.join((message + '\n').encode() for error, message in events if error)
        return out, err


def inventory():
    return tuple([Cell(mode, merged, warnings) for mode in MODES for merged in (False, True) for warnings in (False, True)] +
                 [Cell(mode, merged, child=child, owned=owned) for mode in MODES for merged in (False, True)
                  for child in ('same', 'gnu', 'candidate') for owned in (False, True)] +
                 [Cell(mode, True, child=child, owned=owned, stress=True) for mode in ('target', 'recurse')
                  for child in ('same', 'gnu', 'candidate') for owned in (False, True)])


def materialize(cell, path):
    path.mkdir()
    (path / 'Makefile').write_bytes(cell.source)
    (path / 'helper.py').write_bytes(HELPER)
    if cell.child:
        (path / 'child.mk').write_bytes(b'.PHONY: all\nall:\n\t@python3 helper.py $(BRANCH)1\n\t@python3 helper.py $(BRANCH)2\n')
    if not cell.owned: (path / 'mutex').write_bytes(PROTECTED)


def anchors(cell, observed):
    errors = []
    out, err = cell.expected()
    valid = (out, err)
    if cell.stress:
        # Scheduling may choose either complete target first. Each target must
        # remain contiguous, including all bytes above pipe capacity. No line
        # sorting: an interleaved or truncated group fails this exact validator.
        half = len(out) // 2
        if (observed.stdout, observed.stderr) not in (valid, (out[half:] + out[:half], err)):
            errors.append('complete target group atomicity')
    elif (observed.stdout, observed.stderr) != valid: errors.append('independent output/phase anchor')
    if observed.returncode != 0: errors.append('exit status')
    if observed.limit or observed.leftover_processes: errors.append('process containment')
    return errors


def execute(cell, root, fixture, binary, reference, candidate):
    case = harness.Case(fixture, cell.id, 'runtime.output-sync', 'required', (), 8, {}, {}, 'merged' if cell.merged else 'separate')
    harness.reset_workspace(case, root, binary)
    child = root / 'bin/make' if cell.child == 'same' else reference if cell.child == 'gnu' else candidate
    args = ['-rR', '--no-print-directory', '-j2', '--jobserver-style=pipe', '-O' + cell.mode]
    if not cell.owned: args.append('--sync-mutex=fnm:mutex')
    out, err, status, limit, leftovers = harness.capture([str(root / 'bin/make'), *args], root / 'work',
        harness.clean_environment(root, {'OUTPUT_CHILD': str(child), 'OUTPUT_LARGE': '1' if cell.stress else '0'}), 8, cell.merged)
    return harness.Observation(out, err, status, limit, harness.snapshot({n: root/n for n in ('work','home','tmp')}), leftovers)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--repeat', type=int, default=3)
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--artifacts', type=Path, default=ROOT / 'artifacts/output-sync')
    args = parser.parse_args()
    if bool(args.candidate) == args.self_test: parser.error('choose candidate or self-test')
    if not 1 <= args.repeat <= 100: parser.error('repeat must be 1-100')
    reference = args.oracle.resolve()
    profile = oracle.verify(reference)
    candidate = reference if args.self_test else args.candidate.resolve()
    identity = harness.executable_identity(candidate)
    cells = inventory()
    if set(args.case) - {c.id for c in cells}: parser.error('unknown case')
    args.artifacts.mkdir(parents=True, exist_ok=True)
    destination = Path(tempfile.mkdtemp(prefix='output-', dir=args.artifacts.resolve()))
    rows = []
    for cell in cells:
        if args.case and cell.id not in args.case: continue
        fixture = destination / ('input-' + cell.id)
        materialize(cell, fixture)
        for repeat in range(args.repeat):
            root = destination / 'run'; root.mkdir(exist_ok=True)
            baseline = execute(cell, root, fixture, reference, reference, candidate)
            observed = execute(cell, root, fixture, candidate, reference, candidate)
            differences = harness.differences(baseline, observed)
            if cell.stress:
                differences = [field for field in differences if field != 'stdout']
            errors = anchors(cell, baseline) + anchors(cell, observed) + differences
            rows.append({'case': cell.id, 'repeat': repeat, 'errors': errors,
                         'oracle': dataclasses.asdict(baseline), 'candidate': dataclasses.asdict(observed)})
            print('FAIL' if errors else 'PASS', cell.id, repeat, errors, flush=True)
    report = {'oracle_profile': profile, 'candidate_identity': identity, 'runner_sha256': harness.sha256_file(Path(__file__)),
              'results': rows, 'unordered_validator': 'Large-output profiles accept either complete target group first; raw streams retained; all other observations exact.', 'limitations': ['Trusted bounded fixtures, not a security sandbox. Procfs helper barriers require Linux.']}
    (destination / 'report.json').write_text(json.dumps(report, default=lambda x: {'hex': x.hex()} if isinstance(x, bytes) else str(x), indent=2) + '\n')
    print('Report:', destination / 'report.json')
    return int(any(row['errors'] for row in rows))

if __name__ == '__main__': raise SystemExit(main())
