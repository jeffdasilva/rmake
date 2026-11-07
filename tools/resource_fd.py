# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Bounded startup and live fatal cancellation under a Linux descriptor limit."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import signal
import tempfile

import conformance as h
import oracle
import runtime_signals as fixtures

REPO = Path(__file__).resolve().parents[1]
MODELS = REPO / 'tests/resource-fd/models.json'


def file_record(value: str) -> dict:
    data = value.encode()
    return {'mode': 0o644, 'type': 'file', 'size': len(data),
            'sha256': hashlib.sha256(data).hexdigest(),
            'content': base64.b64encode(data).decode()}


def task(pid: int) -> dict:
    fields = (Path('/proc') / str(pid) / 'stat').read_text().rsplit(')', 1)[1].split()
    return {'pid': pid, 'state': fields[0], 'parent': int(fields[1]),
            'group': int(fields[2]), 'start_tick': int(fields[19])}


def nofile(pid: int) -> list[int]:
    row = next(line for line in (Path('/proc') / str(pid) / 'limits').read_text().splitlines()
               if line.startswith('Max open files'))
    return list(map(int, row.removeprefix('Max open files').split()[:2]))


def private_descriptors(pid: int) -> tuple[dict, list]:
    proc = Path('/proc') / str(pid)
    descriptors = {}
    pipes = {}
    for path in (proc / 'fd').iterdir():
        fd = int(path.name)
        if fd <= 2:
            continue
        target = os.readlink(path)
        info = (proc / 'fdinfo' / path.name).read_text()
        flags = int(next(line.split()[1] for line in info.splitlines() if line.startswith('flags:')), 8)
        descriptors[fd] = {'target': target, 'flags': flags}
        if target.startswith('pipe:['):
            pipes.setdefault(target, []).append(fd)
    return descriptors, sorted(sorted(fds) for fds in pipes.values())


def observe(case, runtime, binary, model, inputs, candidate):
    h.reset_workspace(case, runtime, binary)
    roots = {name: runtime / name for name in ('bin', 'work', 'home', 'tmp')}
    work = roots['work']
    expected = h.snapshot(roots)
    expected['bin/make'] = {'type': 'symlink', 'mode': 0o777, 'target': str(binary)}
    for name, value in inputs['expected_final_markers'].items():
        expected['work/' + name] = file_record(value)
    if model['forwarded_signal'] is not None:
        expected['work/received.signal'] = file_record(model['forwarded_signal'])
    events, errors = [], []
    proof = {}
    number = model['signal']
    limit = model['nofile']

    def inspect(parent, _stdout):
        if not events and (work / 'ready.pid').exists():
            child = int((work / 'ready.pid').read_text())
            proof['make'] = task(parent)
            proof['recipe'] = task(child)
            proof['make_nofile'] = nofile(parent)
            proof['recipe_nofile'] = nofile(child)
            proof['descriptors'], proof['private_pipe_fds'] = private_descriptors(parent)
            proof['caught_before_delivery'] = fixtures.caught(parent, number)
            proof['live_owned_bytes'] = (work / 'out').read_bytes().hex()
            if (proof['recipe']['parent'] != parent or proof['recipe']['group'] != parent
                    or proof['recipe']['state'] in {'Z', 'X'}):
                errors.append('recipe is not a live owned child')
            if any(proof[key] != [limit, limit] for key in ('make_nofile', 'recipe_nofile')):
                errors.append('independent live descriptor limit')
            if not proof['caught_before_delivery']:
                errors.append('make did not install the selected handler')
            if bytes.fromhex(proof['live_owned_bytes']) != inputs['live_target'].encode():
                errors.append('owned target was not written before delivery')
            if limit == 6 and proof['private_pipe_fds']:
                errors.append('live make retained pipe descriptors under pressure')
            if candidate and proof['private_pipe_fds'] != model['candidate_private_pipe']:
                errors.append('candidate wake topology differs from declared control')
            (work / 'ready.pid').unlink()
            (work / 'ready').write_text(inputs['expected_final_markers']['ready'])
            events.append('live-owned-target')
            os.kill(parent, number)
            events.append('signal-to-make')
        if len(events) != 2:
            return
        signal_path = work / 'received.signal'
        actual = signal_path.read_text() if signal_path.exists() else None
        if (not (work / 'out').exists() and not fixtures.caught(parent, number)
                and actual == model['forwarded_signal']):
            try:
                child_now = task(proof['recipe']['pid'])
            except FileNotFoundError:
                errors.append('recipe disappeared before controlled release')
            else:
                if (child_now['start_tick'] != proof['recipe']['start_tick']
                        or child_now['parent'] != parent or child_now['state'] in {'Z', 'X'}):
                    errors.append('recipe was no longer owned and live at release')
            events.append('target-removed-before-release')
            (work / 'release').write_text(inputs['expected_final_markers']['release'])
            events.append('release-child')

    command = ['/usr/bin/prlimit', f'--nofile={limit}:{limit}', '--',
               str(runtime / 'bin/make'), *inputs['make_args']]
    out, err, status, bound, leftovers = h.capture(
        command, work, h.clean_environment(runtime), case.timeout, observe=inspect)
    actual = h.snapshot(roots)
    if proof.get('recipe'):
        try:
            remaining = task(proof['recipe']['pid'])
        except FileNotFoundError:
            proof['same_recipe_remaining'] = False
        else:
            proof['same_recipe_remaining'] = remaining['start_tick'] == proof['recipe']['start_tick']
        if proof['same_recipe_remaining']:
            errors.append('recipe identity remains after make exits')
    errors.extend(name for name, bad in (
        ('stdout', out != model['stdout'].encode()), ('stderr', err != model['stderr'].encode()),
        ('status', status != model['status']), ('bounds/leftovers', bool(bound or leftovers)),
        ('full filesystem', actual != expected),
        ('controlled delivery and cleanup order', events != inputs['expected_events']),
    ) if bad)
    return {'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status,
            'bound': bound, 'leftovers': leftovers, 'filesystem': actual,
            'expected_filesystem': expected, 'events': events, 'proof': proof, 'errors': errors}


def observe_startup(case, runtime, binary, model):
    h.reset_workspace(case, runtime, binary)
    roots = {name: runtime / name for name in ('bin', 'work', 'home', 'tmp')}
    expected = h.snapshot(roots)
    expected['bin/make'] = {'type': 'symlink', 'mode': 0o777, 'target': str(binary)}
    limit = model['nofile']
    command = ['/usr/bin/prlimit', f'--nofile={limit}:{limit}', '--',
               str(runtime / 'bin/make'), *model['args']]
    out, err, status, bound, leftovers = h.capture(
        command, roots['work'], h.clean_environment(runtime), case.timeout)
    filesystem = h.snapshot(roots)
    errors = [name for name, wrong in (
        ('stdout', out.hex() != model['stdout_hex']),
        ('stderr', err.hex() != model['stderr_hex']),
        ('status', status != model['status']),
        ('bounds/leftovers', bool(bound or leftovers)),
        ('full filesystem', filesystem != expected),
    ) if wrong]
    return {'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status,
            'bound': bound, 'leftovers': leftovers, 'filesystem': filesystem,
            'expected_filesystem': expected, 'events': [], 'errors': errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=REPO / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, default=REPO / 'artifacts/resource-fd/campaign.json')
    parser.add_argument('--case', action='append', default=[])
    args = parser.parse_args()
    if args.oracle_only == (args.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    reference = args.oracle.resolve()
    profile = oracle.verify(reference)
    model_data = json.loads(MODELS.read_bytes())
    if model_data['schema_version'] != 1:
        raise h.HarnessError('unsupported resource descriptor model schema')
    inputs = model_data['stable_inputs']
    rows = []
    binaries = [reference, reference] + ([args.candidate.resolve()] if args.candidate else [])
    identities = {'oracle': h.executable_identity(reference, oracle=True)}
    if args.candidate is not None:
        identities['candidate'] = h.executable_identity(binaries[2])
    models = [(True, model) for model in model_data['startup']]
    models += [(False, model) for model in model_data['models']]
    names = {model['id'] for _, model in models}
    if len(names) != len(models) or set(args.case) - names or not models:
        raise h.HarnessError('duplicate, unknown, or empty resource descriptor case selection')
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-low-fd-cancel-') as temporary:
            root = Path(temporary)
            runtime = root / 'runtime'
            runtime.mkdir()
            for startup, model in models:
                if args.case and model['id'] not in args.case:
                    continue
                path = root / model['id']
                if startup:
                    path.mkdir()
                    for name, contents in model.get('files', {'Makefile': 'all:\n'}).items():
                        relative = Path(name)
                        if relative.is_absolute() or not relative.parts or '..' in relative.parts:
                            raise h.HarnessError('invalid startup fixture path')
                        target = path / relative
                        target.parent.mkdir(parents=True, exist_ok=True)
                        target.write_bytes(contents.encode())
                else:
                    signal_case = fixtures.Cell(signal.Signals(model['signal']), 'parent')
                    fixtures.materialize(signal_case, path)
                h.validate_fixture(path)
                case = h.Case(path, model['id'], 'resource.cancellation', 'required',
                              tuple(model['args'] if startup else inputs['make_args']), 6.0, {}, {})
                observations = [
                    observe_startup(case, runtime, binary, model) if startup
                    else observe(case, runtime, binary, model, inputs, index >= 2)
                    for index, binary in enumerate(binaries)
                ]
                repeat_fields = ('stdout_hex', 'stderr_hex', 'status', 'bound',
                                 'leftovers', 'filesystem', 'events')
                repeat = all(observations[0][key] == observations[1][key] for key in repeat_fields)
                gnu2 = repeat and all(not row['errors'] for row in observations[:2])
                passed = gnu2 and all(not row['errors'] for row in observations)
                rows.append({'model': model, 'gnu2_qualified': gnu2, 'repeat_exact': repeat,
                             'passed': passed, 'observations': observations})
                print(model['id'], 'GNU2', gnu2, 'pass', passed,
                      [row['errors'] for row in observations], flush=True)
    finally:
        os.umask(previous_umask)
    totals = {'cases': len(rows), 'gnu2': sum(row['gnu2_qualified'] for row in rows),
              'pass': sum(row['passed'] for row in rows), 'fail': sum(not row['passed'] for row in rows)}
    report = {'oracle_profile': profile['profile_id'], 'identities': identities,
              'passed': not totals['fail'], 'totals': totals,
              'model_sha256': h.sha256_file(MODELS), 'driver_sha256': h.sha256_file(Path(__file__)),
              'fixture_source_sha256': h.sha256_file(Path(fixtures.__file__)),
              'harness_sha256': h.sha256_file(Path(h.__file__)),
              'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
              'binaries': {str(path): h.sha256_file(path) for path in binaries}, 'records': rows}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open('w') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print('Resource descriptor gate', totals, flush=True)
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
