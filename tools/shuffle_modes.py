#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Narrow random/zero-seed validation, separate from exact conformance credit."""
import argparse
import base64
import dataclasses
import itertools
import json
from pathlib import Path
import re
import tempfile

import conformance as h
import debug_output as debug
import oracle
from remake_parallel import HeldModel, execute, release_errors

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (b'$(file >seen,anchor)\n'
          b'$(info SEED=[$(filter --shuffle=%,$(MAKEFLAGS))])\n'
          b'.PHONY: all a b c\nall:a b c\n'
          b'\t@$(file >>seen,all)$(info ALL=[$+]|[$^]):\n'
          b'a:;@$(file >>seen,a):\nb:;@$(file >>seen,b):\nc:;@$(file >>seen,c):\n')


def leaf_order(seed):
    """Independent arithmetic model: one goal draw, then three list swaps."""
    result = [b'a', b'b', b'c']
    state = seed
    for index in range(4):
        for distance in (13, -17, 5):
            state ^= ((state * 2 ** distance) % 2 ** 32 if distance > 0
                      else state // 2 ** -distance)
        state %= 2 ** 32
        if index:
            partner = state % 3
            result[index - 1], result[partner] = result[partner], result[index - 1]
    return result


def validate(observation, zero):
    assert observation.returncode == 0 and not observation.limit
    assert not observation.leftover_processes and observation.stderr == b''
    match = re.fullmatch(rb'SEED=\[--shuffle=([0-9]+)\]\nALL=\[a b c\]\|\[a b c\]\n',
                         observation.stdout)
    assert match, 'seed metadata or exact declaration-order automatic variables'
    seed = int(match[1])
    assert 0 <= seed < 2 ** 32
    if zero:
        assert seed == 0
    seen = base64.b64decode(observation.filesystem['work/seen']['content'])
    lines = seen.splitlines()
    assert lines[0] == b'anchor' and lines[-1] == b'all'
    assert sorted(lines[1:-1]) == [b'a', b'b', b'c'], 'permutation/topological witness'
    if seed:
        assert lines[1:-1] == leaf_order(seed), 'independent numeric permutation'
    return seed



def pattern_wait_cases(reference, candidate, root):
    worker = (ROOT / 'tests/remake-parallel/worker.py').read_bytes()
    anchor = b'anchor\n'
    leaves = ' '.join(f'd{index:02}' for index in range(1, 12))
    models = (
        ('trailing', 'none', 'd01', 'p1 .WAIT', '', b'd01\n', b'd01\nleft\n'),
        ('between', 'none', 'd01', 'p1 .WAIT p2', '', b'', b'left\np2\nd01\n'),
        ('explicit', 'none', 'p1 .WAIT p2 d01', '', '', b'', b'left\np2\nd01\n'),
        ('order-only-transition', 'none', 'd01', 'p1 .WAIT | p2', '', b'p2\nd01\n', b'p2\nd01\nleft\n'),
        ('split-secondary', 'none', 'd01', 'p1 $$(LATE) p2', '.WAIT', b'p2\nd01\n', b'p2\nd01\nleft\n'),
        ('whole-secondary', 'none', 'd01', '$$(LATE)', 'p1 .WAIT p2', b'', b'left\np2\nd01\n'),
        ('retain-seeded', '12345', leaves, 'p1 .WAIT p2', '', b'',
         b'left\np2\nd05\nd04\nd11\nd07\nd01\nd06\nd10\nd08\nd09\nd03\nd02\n'),
        ('retain-reverse', 'reverse', leaves, 'p1 .WAIT p2', '', b'',
         b'left\np2\nd11\nd10\nd09\nd08\nd07\nd06\nd05\nd04\nd03\nd02\nd01\n'),
    )
    scenarios = []
    for name, seed, explicit, pattern, late, held_effects, effects in models:
        prefix = f'.SECONDEXPANSION:\nLATE = {late}\n' if late else ''
        declared_leaves = ' '.join(name for name in explicit.split() if name.startswith('d'))
        makefile = (prefix + f'all.out: {explicit}\n%.out: {pattern} ; @$(file >>seen,goal):\n'
                    f'p1: ; @python3 worker.py left observe\np2 {declared_leaves}: ; @$(file >>seen,$@):\n')
        scenarios.append((name, seed, makefile, held_effects, effects))
    targeted = (
        ('single-original', 'all.out: a\n', b'p2\na\n', b'p2\na\nleft\n'),
        ('two-original', 'all.out: a b\n', b'p2\na\n', b'p2\na\nleft\nb\n'),
        ('one-deferred', 'all.out: a $$(DYNAMIC)\n', b'p2\na\nx\ny\n', b'p2\na\nx\ny\nleft\n'),
        ('deferred-after-immediate', 'all.out: a\nall.out: $$(DYNAMIC)\n',
         b'p2\na\nx\ny\n', b'p2\na\nx\ny\nleft\n'),
        ('deferred-before-immediate', 'all.out: $$(DYNAMIC)\nall.out: a\n',
         b'p2\nx\ny\n', b'p2\nx\ny\nleft\na\n'),
    )
    for name, explicit, held_effects, effects in targeted:
        makefile = ('.SECONDEXPANSION:\n.NOTPARALLEL: all.out\nDYNAMIC := x y\n' + explicit
                    + '%.out: p1 p2 ; @$(file >>seen,goal):\n'
                    'p1: ; @python3 worker.py left observe\np2 a b x y: ; @$(file >>seen,$@):\n')
        scenarios.append(('targeted-' + name, 'none', makefile, held_effects, effects))
    double_colon = (
        ('head-deferred', 'p1 $$(DYNAMIC)', 'z', b'x\ny\n', b'x\ny\nleft\nhead\nz\n'),
        ('tail-deferred', 'z', 'p1 $$(DYNAMIC)', b'z\nhead\nx\ny\n', b'z\nhead\nx\ny\nleft\n'),
        ('head-static', 'p1 x y', 'z', b'', b'left\nx\ny\nhead\nz\n'),
        ('tail-static', 'z', 'p1 x y', b'z\nhead\n', b'z\nhead\nleft\nx\ny\n'),
    )
    for name, head, tail, held_effects, effects in double_colon:
        makefile = ('.SECONDEXPANSION:\n.NOTPARALLEL: all.out\nDYNAMIC := x y\n'
                    f'all.out:: {head} ; @$(file >>seen,head):\n'
                    f'all.out:: {tail} ; @$(file >>seen,goal):\n'
                    'p1: ; @python3 worker.py left observe\nx y z: ; @$(file >>seen,$@):\n')
        scenarios.append(('targeted-double-' + name, 'none', makefile, held_effects, effects))
    both = 'all.out peer.out'
    grouped_double = (
        ('head-both-static', both, 'p1 x y', 'z', b'', b'left\nx\ny\nhead\nz\n'),
        ('head-trigger-only', 'all.out', 'p1 x y', 'z', b'x\ny\n', b'x\ny\nleft\nhead\nz\n'),
        ('head-peer-only', 'peer.out', 'p1 x y', 'z', b'x\ny\n', b'x\ny\nleft\nhead\nz\n'),
        ('head-both-deferred', both, 'p1 $$(DYNAMIC)', 'z', b'x\ny\n', b'x\ny\nleft\nhead\nz\n'),
        ('tail-both-static', both, 'z', 'p1 x y', b'z\nhead\n', b'z\nhead\nleft\nx\ny\n'),
        ('tail-both-deferred', both, 'z', 'p1 $$(DYNAMIC)', b'z\nhead\nx\ny\n', b'z\nhead\nx\ny\nleft\n'),
    )
    for name, targets, head, tail, held_effects, effects in grouped_double:
        makefile = (f'.SECONDEXPANSION:\n.NOTPARALLEL: {targets}\nDYNAMIC := x y\n'
                    f'all.out peer.out &:: {head} ; @$(file >>seen,head):\n'
                    f'all.out peer.out &:: {tail} ; @$(file >>seen,goal):\n'
                    'p1: ; @python3 worker.py left observe\nx y z: ; @$(file >>seen,$@):\n')
        scenarios.append(('targeted-grouped-double-' + name, 'none', makefile, held_effects, effects))
    group_recipe = ' ; @$(file >>seen,goal):\n'
    grouped_single = (
        ('both-static', both, 'all.out peer.out &: p1 x y' + group_recipe, b'', b'left\nx\ny\n'),
        ('trigger-only', 'all.out', 'all.out peer.out &: p1 x y' + group_recipe, b'x\ny\n', b'x\ny\nleft\n'),
        ('peer-only', 'peer.out', 'all.out peer.out &: p1 x y' + group_recipe, b'x\ny\n', b'x\ny\nleft\n'),
        ('both-deferred', both, 'all.out peer.out &: p1 $$(DYNAMIC)' + group_recipe,
         b'x\ny\n', b'x\ny\nleft\n'),
        ('peer-disjoint', 'peer.out', 'all.out peer.out &:' + group_recipe + 'all.out: p1\npeer.out: x y\n',
         b'x\n', b'x\nleft\ny\n'),
        ('trigger-disjoint', 'all.out', 'all.out peer.out &:' + group_recipe + 'all.out: p1 x\npeer.out: y\n',
         b'y\n', b'y\nleft\nx\n'),
        ('implicit-only', 'all.out', '%.out %.peer &: p1 x y' + group_recipe, b'x\ny\n', b'x\ny\nleft\n'),
        ('three-peers', 'all.out last.out', 'all.out peer.out last.out &: p1 x y' + group_recipe,
         b'x\ny\n', b'x\ny\nleft\n'),
    )
    for name, targets, declarations, held_effects, effects in grouped_single:
        makefile = (f'.SECONDEXPANSION:\n.NOTPARALLEL: {targets}\nDYNAMIC := x y\n' + declarations
                    + 'p1: ; @python3 worker.py left observe\nx y: ; @$(file >>seen,$@):\n')
        scenarios.append(('targeted-grouped-single-' + name, 'none', makefile, held_effects, effects))
    records = []
    runtime = root / 'observation'
    for (name, seed, makefile, held_effects, effects), transport in itertools.product(
            scenarios, ('pipe', 'fifo')):
        ident = 'pattern-wait-' + name + '-' + transport
        model = debug.Model(
            ident, {'Makefile': makefile.encode(), 'worker.py': worker, 'seen': anchor},
            ('-rR', '--no-print-directory', f'--shuffle={seed}', '-j2',
             f'--jobserver-style={transport}', 'all.out'), {},
            {'exit_code': 0, 'stdout_template_hex': '', 'stderr_template_hex': ''},
            anchor + effects + b'goal\n',
            updates={'left.started': b'started\n', 'left.release': b'yes\n', 'left.done': b'done\n'},
        )
        held = HeldModel(model, overlap=False, before_second=None, before_first=anchor + held_effects)
        case, filesystem = debug.prepare(model, root / ident, runtime)
        observations, releases = zip(*(execute(case, runtime, binary) for binary in (
            reference, reference, candidate)))
        result = debug.assess(model, case, runtime, filesystem, list(observations))
        errors = [release_errors(held, release) for release in releases]
        result.update(id=ident, releases=releases, release_errors=errors,
                      passed=result['passed'] and not any(errors))
        records.append(result)
    return records


def run(reference, candidate, report):
    records = []
    with tempfile.TemporaryDirectory(prefix='rmake-shuffle-random-') as directory:
        root = Path(directory)
        (root / 'observation').mkdir()
        source = root / 'source'
        source.mkdir()
        (source / 'Makefile').write_bytes(SOURCE)
        # Each engine keeps its own nondeterministic seed. Nothing masks a
        # deterministic edge order, status, filesystem or replay difference.
        for mode, jobs, transport, origin in itertools.product(
                ('random', 'bare', 'zero'), (1, 4), ('fifo', 'pipe'), ('cli', 'gnu', 'make')):
            option = '--shuffle' if mode == 'bare' else '--shuffle=' + ('0' if mode == 'zero' else mode)
            args = ['-rR', '--no-print-directory', '-j' + str(jobs), '--jobserver-style=' + transport]
            environment = {'GNUMAKEFLAGS': ''}
            if origin == 'cli':
                args.append(option)
            else:
                environment['GNUMAKEFLAGS' if origin == 'gnu' else 'MAKEFLAGS'] = option
            case = h.Case(source, f'{mode}-j{jobs}-{transport}-{origin}',
                          'options.shuffle.semantic', 'required', tuple(args + ['all']), 5,
                          environment, {}, 'separate')
            for engine, binary in (('oracle', reference), ('candidate', candidate)):
                observations = []
                for repeat in range(2):
                    observation = h.execute(case, root / 'observation', binary)
                    seed = validate(observation, mode == 'zero')
                    entry = dict(seed=seed, observation=h.observation_json(observation))
                    if seed:
                        replay = dataclasses.replace(
                            case, args=tuple(args[:4] + ['--shuffle=' + str(seed), 'all']),
                            environment={'GNUMAKEFLAGS': ''})
                        replayed = h.execute(replay, root / 'observation', binary)
                        assert not h.differences(observation, replayed), 'same-engine fixed-seed replay'
                        entry['replay'] = h.observation_json(replayed)
                    observations.append(entry)
                records.append(dict(id=case.id, engine=engine, observations=observations))
        barriers = pattern_wait_cases(reference, candidate, root)
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps(dict(
        classification='Narrow semantic random/zero-seed checks; zero exact-conformance or safety-exception credit.',
        identities={str(binary): h.sha256_file(binary) for binary in (reference, candidate)},
        source_hex=SOURCE.hex(), records=records, pattern_wait=barriers), indent=2) + '\n')
    print(f'shuffle modes: {len(records)} engine cells, repeated bounded observations and nonzero-seed replay pass')
    passed = sum(result['passed'] for result in barriers)
    print(f'prerequisite barriers: {passed}/{len(barriers)} held profiles passed')
    return int(passed != len(barriers))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/shuffle-modes/report.json')
    args = parser.parse_args()
    oracle.verify(args.oracle.resolve())
    return run(args.oracle.resolve(), args.candidate.resolve(), args.report)


if __name__ == '__main__':
    raise SystemExit(main())
