#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original CLI exit-phase controls with narrowly calibrated cosmetic banners."""
from __future__ import annotations
import argparse
import base64
import dataclasses
import json
from pathlib import Path
import tempfile
import conformance as harness
import oracle

ROOT = Path(__file__).resolve().parents[1]
SOURCE = b'$(error READER MUST NOT RUN)\nall:;@:\n'
PREFIX = b'BEFORE\nmake: WARN\n'
FATAL = b'make: *** STOP.  Stop.\n'

@dataclasses.dataclass(frozen=True)
class Cell:
    action: str
    position: str
    fatal: bool
    inherited: bool
    mode: str

    @property
    def id(self):
        return f'{self.action}-{self.position}-{"fatal" if self.fatal else "success"}-{"env" if self.inherited else "cli"}-{self.mode}'

    @property
    def flags(self):
        return {'help': ['--help'], 'version': ['--version'],
                'help-version': ['--help', '--version'],
                'version-help': ['--version', '--help']}[self.action]

    @property
    def args(self):
        effect = 'A:=$(info BEFORE)$(warning WARN)$(file >>seen,E)'
        if self.fatal: effect += '$(error STOP)'
        arguments = [*self.flags, effect] if self.position == 'before' else [effect, *self.flags]
        return ['-rR', '--no-print-directory', *([] if self.inherited else ['-O' + self.mode]),
                *arguments, 'AFTER:=$(file >>after,E)', '-E', '$(file >eval-forbidden,E)']

    def expected(self, banners):
        if self.fatal:
            banner = banners['version'] if self.position == 'before' and 'version' in self.action else b''
            return (banner + PREFIX + FATAL if self.inherited and self.mode in ('line','target') else PREFIX + FATAL + banner)
        banner = banners['help'] if 'help' in self.action else b''
        if 'version' in self.action:
            banner = banners['version'] + (b'\n' + banner if banner else b'')
        if self.inherited and self.mode in ('line', 'target'):
            return banner + PREFIX
        return PREFIX + banner


def inventory():
    return tuple(Cell(action, position, fatal, inherited, mode)
                 for action in ('help', 'version', 'help-version', 'version-help')
                 for position in ('before', 'after') for fatal in (False, True)
                 for inherited in (False, True) for mode in ('none', 'line', 'target', 'recurse'))


def banners_for(root, fixture, binary):
    result = {}
    for kind in ('help', 'version'):
        case = harness.Case(fixture, 'banner-' + kind, 'cli.startup-exits', 'required',
                            ('--' + kind,), 5, {}, {})
        observed = harness.execute(case, root, binary)
        if (observed.returncode != 0 or observed.stderr or observed.limit or observed.leftover_processes
                or not observed.stdout or b'READER MUST NOT RUN' in observed.stdout
                or b'BEFORE' in observed.stdout or b'WARN' in observed.stdout):
            raise harness.HarnessError('invalid isolated cosmetic banner: ' + kind)
        if (set(observed.filesystem) != {'home','tmp','work','work/Makefile'}
                or observed.filesystem['work/Makefile'].get('content') != base64.b64encode(SOURCE).decode()):
            raise harness.HarnessError('isolated banner changed files: ' + kind)
        result[kind] = observed.stdout
    return result


def anchors(cell, observed, banners):
    errors = []
    if observed.returncode != (2 if cell.fatal else 0): errors.append('exit phase status')
    if observed.stdout != cell.expected(banners): errors.append('exact output outside calibrated banner')
    if observed.stderr: errors.append('unexpected separate diagnostic')
    if observed.limit or observed.leftover_processes: errors.append('process containment')
    expected_seen = b'E\n'
    seen = observed.filesystem.get('work/seen', {})
    if seen.get('content') != base64.b64encode(expected_seen).decode(): errors.append('eager CLI effect count')
    if 'work/eval-forbidden' in observed.filesystem: errors.append('eval buffer after early exit')
    after = observed.filesystem.get('work/after', {})
    if cell.fatal:
        if after: errors.append('later argument after fatal')
    elif after.get('content') != base64.b64encode(expected_seen).decode(): errors.append('later argument before early exit')
    return errors


def run_cell(cell, root, fixture, binary, banners):
    case = harness.Case(fixture, cell.id, 'cli.startup-exits', 'required', tuple(cell.args), 5,
                        {'MAKEFLAGS': '-O' + cell.mode} if cell.inherited else {}, {}, 'merged')
    observed = harness.execute(case, root, binary)
    return observed, anchors(cell, observed, banners)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX/'bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--artifacts', type=Path, default=ROOT/'artifacts/startup-exits')
    args = parser.parse_args(argv)
    if bool(args.candidate) == args.self_test: parser.error('choose candidate or self-test')
    reference = args.oracle.resolve(); profile = oracle.verify(reference)
    candidate = reference if args.self_test else args.candidate.resolve()
    cells = inventory()
    if set(args.case) - {c.id for c in cells}: parser.error('unknown case')
    args.artifacts.mkdir(parents=True, exist_ok=True)
    destination = Path(tempfile.mkdtemp(prefix='exit-phases-', dir=args.artifacts.resolve()))
    fixture = destination/'input'; fixture.mkdir(); (fixture/'Makefile').write_bytes(SOURCE)
    rows = []
    with tempfile.TemporaryDirectory(prefix='rmake-startup-exits-') as tmp:
        root = Path(tmp)
        banners = [banners_for(root, fixture, binary) for binary in (reference, candidate)]
        for cell in cells:
            if args.case and cell.id not in args.case: continue
            # Two oracle observations must agree even in the deliberately cosmetic region.
            baseline, errors = run_cell(cell, root, fixture, reference, banners[0])
            repeat, more = run_cell(cell, root, fixture, reference, banners[0]); errors += more
            errors += harness.differences(baseline, repeat)
            observed, more = run_cell(cell, root, fixture, candidate, banners[1]); errors += more
            # Files/status/effects retain exact cross-engine comparison; only the
            # explicitly positioned help/version block has distinct known bytes.
            errors += harness.differences(dataclasses.replace(baseline, stdout=b''), dataclasses.replace(observed, stdout=b''))
            def evidence(value):
                return dict(harness.observation_json(value), stdout_hex=value.stdout.hex(), stderr_hex=value.stderr.hex())
            rows.append(dict(case=cell.id, errors=errors, oracle=evidence(baseline), repeat=evidence(repeat), candidate=evidence(observed)))
            print('FAIL' if errors else 'PASS', cell.id, errors, flush=True)
    report = dict(oracle_profile=profile, candidate_identity=harness.executable_identity(candidate),
                  runner_sha256=harness.sha256_file(Path(__file__)), banners=[{k:v.hex() for k,v in row.items()} for row in banners],
                  results=rows, policy='Only isolated help/version banner bytes differ; their exact multiplicity/position and every other stream/status/file observation are independently anchored.')
    (destination/'report.json').write_text(json.dumps(report, indent=2)+'\n')
    print('Startup exits:',len(rows),'passed:',sum(not r['errors'] for r in rows),'Report:',destination/'report.json')
    return int(any(r['errors'] for r in rows))

if __name__ == '__main__': raise SystemExit(main())
