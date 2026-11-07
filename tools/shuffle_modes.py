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
    models = (
        ('trailing', 'd01', 'p1 .WAIT', '', b'd01\n', b'd01\nleft\n'),
        ('between', 'd01', 'p1 .WAIT p2', '', b'', b'left\np2\nd01\n'),
        ('explicit', 'p1 .WAIT p2 d01', '', '', b'', b'left\np2\nd01\n'),
        ('order-only-transition', 'd01', 'p1 .WAIT | p2', '', b'p2\nd01\n', b'p2\nd01\nleft\n'),
        ('split-secondary', 'd01', 'p1 $$(LATE) p2', '.WAIT', b'p2\nd01\n', b'p2\nd01\nleft\n'),
        ('whole-secondary', 'd01', '$$(LATE)', 'p1 .WAIT p2', b'', b'left\np2\nd01\n'),
    )
    records = []
    runtime = root / 'observation'
    for (name, explicit, pattern, late, held_effects, effects), transport in itertools.product(
            models, ('pipe', 'fifo')):
        ident = 'pattern-wait-' + name + '-' + transport
        prefix = f'.SECONDEXPANSION:\nLATE = {late}\n' if late else ''
        makefile = (prefix + f'all.out: {explicit}\n%.out: {pattern} ; @$(file >>seen,goal):\n'
                    'p1: ; @python3 worker.py left observe\np2 d01: ; @$(file >>seen,$@):\n')
        model = debug.Model(
            ident, {'Makefile': makefile.encode(), 'worker.py': worker, 'seen': anchor},
            ('-rR', '--no-print-directory', '--shuffle=none', '-j2',
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
    print(f'implicit pattern barriers: {passed}/{len(barriers)} held profiles passed')
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
