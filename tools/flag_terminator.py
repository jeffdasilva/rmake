#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact GNU2 models for inherited option terminators, effects and source boundaries."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/flag-terminator/Makefile'


@dataclass(frozen=True)
class Model:
    id: str
    makeflags: bytes = b''
    gnumakeflags: bytes = b''
    args: tuple[bytes, ...] = ()
    directives: bytes = b''
    goals: bytes = b''
    before: bytes = b''
    after: bytes = b''
    targets: tuple[bytes, ...] = (b'all',)
    mode: str = 'normal'
    flags_have: bytes = b''
    special: str = ''


def models():
    yield Model('inherited-short-goal', b'-- -s', goals=b'-s', targets=(b'-s',))
    yield Model('inherited-long-goal', b'-- --trace', goals=b'--trace', targets=(b'--trace',))
    yield Model('inherited-dash-assignment', b'-- -X:=value', before=b'value', after=b'value',
                flags_have=b' -- -X:=value')
    yield Model('inherited-nonoption-order', b'-- -s explicit', args=(b'selected',),
                goals=b'-s explicit selected', targets=(b'-s', b'explicit', b'selected'))
    yield Model('gnu-source-separation', b'-n', b'-- -s', args=(b'selected',),
                goals=b'-s selected', targets=(b'-s', b'selected'), mode='dry', flags_have=b'nrR')
    yield Model('cli-source-separation', b'-- -s', args=(b'-n', b'selected'),
                goals=b'-s selected', targets=(b'-s', b'selected'), mode='dry', flags_have=b'nrR')
    yield Model('marker-option-operand', b'-I -- -s', args=(b'selected',), goals=b'selected',
                targets=(b'selected',), mode='silent', flags_have=b'-I--')
    yield Model('literal-second-marker', b'-- --', goals=b'--', targets=(b'--',))
    yield Model('escaped-marker', rb'\-\- -s', goals=b'-s', targets=(b'-s',))
    yield Model('plain-goal', b'-- explicit', goals=b'explicit', targets=(b'explicit',))
    yield Model('live-ignored-goal', directives=b'MAKEFLAGS = -- -n')
    yield Model('live-dash-assignment', directives=b'MAKEFLAGS = -- -X:=live', after=b'live')
    yield Model('live-operand', directives=b'MAKEFLAGS = -I -- -n', mode='dry', flags_have=b'-I--')
    yield Model('live-source-separation', directives=b'GNUMAKEFLAGS = -- -n\nMAKEFLAGS = -s', mode='silent')
    yield Model('long-operand-inclusion', b'--include-dir -- -s', args=(b'selected',),
                directives=b'include carried.mk', goals=b'selected', after=b'from-directory',
                targets=(b'selected',), mode='silent', flags_have=b'-I--', special='include')
    yield Model('late-long-operand-inclusion', directives=b'MAKEFLAGS = --include-dir -- -s\ninclude carried.mk',
                after=b'from-directory', mode='silent', flags_have=b'-I--', special='include')
    yield Model('contextual-include-operand', directives=b'MAKEFLAGS = -I~ -- -n\ninclude carried.mk',
                after=b'from-home', special='contextual-include')
    yield Model('literal-raw-byte-goal', b'-- -\xff', goals=b'-\xff', targets=(b'-\xff',), special='raw')
    yield Model('explicit-terminator-after-inherited', b'-- -s', args=(b'--', b'--trace'),
                goals=b'-s --trace', targets=(b'-s', b'--trace'))
    yield Model('recursive-dash-definition', b'-- -X:=propagated', before=b'propagated', after=b'propagated',
                flags_have=b' -- -X:=propagated', special='recursive')
    yield Model('actual-restart-with-inherited-goal', b'-- -s', goals=b'-s', targets=(b'-s',), special='restart')


def write(path, data):
    path.parent.mkdir(exist_ok=True, parents=True)
    path.write_bytes(data)


def prepare(runtime, model):
    work, home, temporary = (runtime / name for name in ('work', 'home', 'tmp'))
    for path in (work, home, temporary):
        path.mkdir()
    source = FIXTURE.read_bytes().replace(b'# CASE-DIRECTIVES', model.directives)
    if model.special == 'raw':
        source = source.replace(b'all -s ', b'all -\xff -s ')
    elif model.special == 'include':
        write(work / '--/carried.mk', b'-X := from-directory\n')
    elif model.special == 'contextual-include':
        write(home / 'carried.mk', b'-X := from-home\n')
    elif model.special == 'recursive':
        source += b"all: ; @$(MAKE) --no-print-directory -f Child.make child; printf '%s\\n' 'recipe|all|$(MAKEFLAGS)|$(-X)' > all.out\n"
        source = b'.DEFAULT_GOAL := all\n' + source.replace(b'all -s ', b'-s ')
        write(work / 'Child.make', b"child: ; @printf '%s\\n' 'child|$(MAKECMDGOALS)|$(-X)|$(origin -X)' > child.out\n")
    elif model.special == 'restart':
        source += (b"include generated.mk\n"
                   b"generated.mk:\n\t@printf '%s\\n' 'RESTARTED := present' > generated.mk\n"
                   b"$(info restart|$(MAKE_RESTARTS)|$(RESTARTED))\n")
    write(work / 'Makefile', source)
    return work, home, temporary


def observe(model, binary, invoked, runtime):
    work, home, temporary = prepare(runtime, model)
    invoked.unlink(missing_ok=True)
    invoked.symlink_to(binary)
    env = oracle.clean_environment(home) | {'TMPDIR': str(temporary), 'MAKEFLAGS': model.makeflags,
                                              'GNUMAKEFLAGS': model.gnumakeflags}
    out, err, status, limit, children = h.capture([str(invoked), '-rR', '--no-print-directory', *model.args],
                                                  work, env, 10)
    files = h.snapshot({'work': work, 'home': home, 'tmp': temporary})
    expectations = [
        (b'pre|' + model.goals + b'|' + model.before + b'\n') in out,
        (b'post|' + model.goals + b'|' + model.after + b'\n') in out,
        not err and (status, limit, children) == (0, None, False),
    ]
    output_files = {os.fsencode(path.name): path.read_bytes() for path in work.glob('*.out')}
    target_files = {target + b'.out' for target in model.targets}
    if model.special == 'recursive':
        expectations.append(output_files.get(b'child.out') == b'child|child|propagated|command line\n')
        target_files.add(b'child.out')
    if model.special == 'restart':
        expectations.extend([b'restart||\n' in out, b'restart|1|present\n' in out,
                             (work / 'generated.mk').read_bytes() == b'RESTARTED := present\n'
                             if (work / 'generated.mk').is_file() else False])
    if model.mode == 'dry':
        expectations.extend([not output_files, model.flags_have in out,
                             all(b'recipe|' + target + b'|' in out for target in model.targets)])
    else:
        expectations.extend([set(output_files) == target_files,
                             all((data := output_files.get(target + b'.out', b'')).startswith(
                                 b'recipe|' + target + b'|') and model.flags_have in data and
                                 data.endswith(b'|' + model.after + b'\n') for target in model.targets)])
        if model.special != 'recursive':
            expectations.append((b"printf '%s" not in out) == (model.mode == 'silent'))
    return {'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status, 'limit': limit,
            'leftover_processes': children, 'files': files, 'model_pass': all(expectations),
            'failed_model_positions': [i for i, passed in enumerate(expectations) if not passed]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case')
    args = parser.parse_args()
    selected = [model for model in models() if not args.case or args.case == model.id]
    if not selected:
        parser.error(f'unknown case: {args.case}')
    profile = oracle.verify(args.oracle)
    results = []
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-flag-terminator-') as directory:
            root = Path(directory)
            runtime = root / 'runtime'
            for model in selected:
                observations = []
                for engine in (args.oracle, args.oracle, args.candidate):
                    runtime.mkdir()
                    try:
                        observations.append(observe(model, engine.absolute(), root / 'make', runtime))
                    finally:
                        h.remove_tree(runtime)
                gnu2 = observations[0]['model_pass'] and observations[0] == observations[1]
                passed = gnu2 and observations[2] == observations[0]
                print('PASS' if passed else 'FAIL', model.id, 'GNU2', gnu2,
                      [r['failed_model_positions'] for r in observations], flush=True)
                results.append({'id': model.id, 'gnu2': gnu2, 'pass': passed, 'observations': observations})
    finally:
        os.umask(previous_umask)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    passed = sum(row['pass'] for row in results)
    args.report.write_text(json.dumps({'cases': len(results), 'passed': passed, 'results': results,
                                      'oracle_profile': profile['profile_id'],
                                      'candidate_sha256': h.sha256_file(args.candidate),
                                      'fixture_sha256': h.sha256_file(FIXTURE)}, indent=2) + '\n')
    print(f'Flag terminator gate: {passed}/{len(results)}', flush=True)
    return int(passed != len(results))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (h.HarnessError, oracle.OracleError, OSError) as error:
        raise SystemExit(f'flag terminator gate: {error}') from error
