#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Literal controller phase and controlled load debug comparison."""
from dataclasses import dataclass, replace
import argparse
import json
import os
from pathlib import Path
import shutil
import tempfile

import conformance as h
import debug_verbose as verbose
import job_debug as jobs
import load_limits
import nls_diagnostics as nls

PIPE = jobs.ROOT / 'tests/job-debug/inherited_pipe.c'
BARRIER_INDEX = 7


@dataclass(frozen=True)
class Load:
    name: str
    language: str
    proc: bytes
    mode: str
    average: str
    trace: bytes
    lines: tuple[bytes, ...]
    limit: str = '100'


def load_models():
    proc = b'Using /proc/loadavg load detection method.\n'
    system = b'Using system load detection method.\n'
    running = b'Running: system = 3 / make = 1 (max requested = 100.000000)\n'
    estimate = b'Estimated system load = 1.250000 (actual = 1.250000) (max requested = 100.000000)\n'
    record = b'99 88 77 3/100 1\n'
    return (
        Load('proc', 'C', record, 'normal', '1.25', b'proc\n', (proc, running)),
        Load('proc-retry', 'C', record, 'read-eintr', '1.25', b'proc\nread-fault\n', (proc, running)),
        Load('proc-fr', 'fr_FR.UTF-8', record, 'normal', '1,25', b'proc\n',
             (proc, running.replace(b'100.', b'100,'))),
        Load('open-fallback', 'C', b'', 'open-denied', '1.25', b'proc\naverage\n', (system, estimate)),
        Load('open-fallback-fr', 'fr_FR.UTF-8', b'', 'open-denied', '1,25', b'proc\naverage\n',
             (system, estimate.replace(b'1.25', b'1,25').replace(b'100.', b'100,'))),
        Load('malformed-fallback', 'C', b'bad \xff load record\n', 'normal', '1.25',
             b'proc\naverage\n', (proc, b'Failed to parse /proc/loadavg: bad \xff load record\n', b'\n', estimate)),
        Load('read-fallback', 'C', record, 'read-eio', '1.25', b'proc\nread-fault\naverage\n',
             (proc, b'Failed to read /proc/loadavg: Input/output error\n', estimate)),
        Load('disabled', 'C', record, 'normal', '1.25', b'', (), '-2'),
    )


def controller_models():
    data = json.loads(verbose.MODELS.read_bytes())
    original = next(model for model in data['models'] if model['id'] == 'empty-no-recipe')
    for name in ('owned', 'inherited', 'disabled'):
        for language, result in original['expected'].items():
            after_reading = ('Updating makefiles....\n' if language == 'C' else 'Mise à jour des makefiles....\n')
            before, after = result['stdout'].split(after_reading, 1)
            auth = {'owned': '3,4', 'inherited': '20,21'}.get(name)
            controller = f'Using jobserver controller {auth}\n' if auth else ''
            expected = {**result, 'stdout': before + 'original reader marker\n' + controller + after_reading + after}
            yield name, language, {**original, 'makefile': '$(info original reader marker)\n' + original['makefile']}, expected


def compile_fixture(source, root):
    cc = shutil.which('cc')
    if cc is None:
        raise h.HarnessError('test fixture requires cc')
    target = root / (source.stem + '.so')
    command = [cc, '-shared', '-fPIC', '-O2', '-Wall', '-Wextra', '-Werror', str(source), '-ldl', '-o', str(target)]
    out, err, code, limit, leftovers = h.capture(command, root, h.clean_environment(root), 30)
    if code or limit or leftovers:
        raise h.HarnessError('fixture compilation failed: ' + repr(err))
    return target, {'command': command, 'source_sha256': h.sha256_file(source), 'output_sha256': h.sha256_file(target),
                    'stdout_hex': out.hex(), 'stderr_hex': err.hex()}


def controller_run(name, language, model, expected, root, runtime, engines, environment, helper):
    fixture = root / f'controller-{name}-{language}'
    verbose.prepare(fixture, model)
    env = dict(environment)
    args = ('-rR', '--no-print-directory', *model['args'])
    if name == 'owned':
        args += ('-j2', '--jobserver-style=pipe')
    elif name == 'inherited':
        env.update(LD_PRELOAD=str(helper), MAKEFLAGS='-j2 --jobserver-auth=20,21')
    case = h.Case(fixture, fixture.name, 'debug.jobs.controller', 'required', args, 5, env, {})
    h.reset_workspace(case, runtime, engines[0])
    fs = h.snapshot({part: runtime / part for part in ('work', 'home', 'tmp')})
    row = {'id': fixture.name, 'args': args, 'environment': env,
           'expected': {**expected, 'filesystem': fs}, 'observations': []}
    for executable in engines:
        observed = verbose.observation(case, runtime, executable, model, row['expected'])
        row['observations'].append(observed)
        if observed['limit'] or observed['leftover_processes'] or observed['returncode'] < 0:
            row['zero_credit'] = True
            break
    row['passed'] = not any(item['differences'] for item in row['observations'])
    return row


def assess_load(case, observation, model, host, runtime, initial, protocol):
    # Only these declared lines at their fixed event positions are partitioned.
    # The rest goes through the exact existing JOBS identity and FS verifier.
    actual = observation.stdout.splitlines(keepends=True)
    end = BARRIER_INDEX + len(case.lines)
    if tuple(actual[BARRIER_INDEX:end]) != case.lines:
        return ['fixed load events'] + jobs.assess(
            model, observation, host, runtime, initial, protocol)
    child_stream = b''.join(actual[:BARRIER_INDEX] + actual[end:])
    return jobs.assess(model, replace(observation, stdout=child_stream), host, runtime, initial, protocol)


def load_run(case, root, runtime, engines, environment, helper):
    model = next(model for model in jobs.models() if model.id == 'jobs-parallel-pipe')
    fixture = root / ('load-' + case.name)
    fixture.mkdir()
    files = {'Makefile': model.source, 'worker.py': jobs.WORKER.read_bytes(),
             'seen': b'anchor\n', 'load.input': case.proc}
    for name, contents in files.items():
        (fixture / name).write_bytes(contents)
    initial = h.snapshot({'work': fixture})
    initial.update({name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp')})
    trace = root / 'load.trace'
    env = {**environment, 'LD_PRELOAD': str(helper), 'RMAKE_TEST_LOAD': case.mode,
           'RMAKE_LOAD_AVERAGE': case.average, 'RMAKE_LOAD_TRACE': str(trace)}
    args = ('-rR', '--no-print-directory', '--debug=j', '-j2', '--jobserver-style=pipe', '-l' + case.limit)
    program_case = h.Case(fixture, fixture.name, 'debug.jobs.load', 'required', args, 6, env, {})
    row = {'id': fixture.name, 'args': args, 'environment': env,
           'input_filesystem': initial, 'fixed_load_events_hex': [line.hex() for line in case.lines],
           'load_events_after_child_event_count': BARRIER_INDEX, 'expected_trace_hex': case.trace.hex(),
           'expected_events': model.events, 'expected_seen_hex': model.seen.hex(), 'observations': []}
    protocol = jobs.ENGLISH if case.language == 'C' else jobs.FRENCH
    for executable in engines:
        trace.unlink(missing_ok=True)
        observation, host = jobs.execute(model, program_case, runtime, executable, protocol)
        actual_trace = trace.read_bytes() if trace.is_file() else b''
        errors = assess_load(case, observation, model, host, runtime, initial, protocol)
        if actual_trace != case.trace:
            errors.append('independent load mechanism')
        row['observations'].append({**h.observation_json(observation), 'stdout_hex': observation.stdout.hex(),
                                    'stderr_hex': observation.stderr.hex(), 'trace_hex': actual_trace.hex(),
                                    'host': host, 'differences': errors})
        if observation.limit or observation.leftover_processes or observation.returncode < 0:
            row['zero_credit'] = True
            break
    row['passed'] = not any(item['differences'] for item in row['observations'])
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=jobs.ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--locale-root', type=Path, required=True)
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--declarations', type=Path)
    args = parser.parse_args()
    oracle, profile, _, locale = nls.prepare_reference(args.oracle, jobs.REFERENCE_FRENCH_CATALOG, args.locale_root)
    controllers = tuple(controller_models())
    loads = load_models()
    if args.declarations:
        with args.declarations.open('x') as output:
            json.dump({'controller': controllers, 'load': [vars(model) for model in loads]}, output,
                      default=lambda value: {'hex': value.hex()}, indent=2)
    engines = [oracle, oracle]
    if args.candidate:
        engines.append(args.candidate.resolve())
    report = {'oracle_profile_id': profile['profile_id'], 'locale_profile': locale,
              'binary_identities': [h.executable_identity(path) for path in engines],
              'source_sha256': h.sha256_file(Path(__file__)),
              'functional_cpu_affinity': sorted(os.sched_getaffinity(0)), 'cases': []}
    with tempfile.TemporaryDirectory(prefix='rmake-job-debug-load-') as directory:
        root = Path(directory)
        runtime = root / 'runtime'
        runtime.mkdir()
        load_helper, load_compile = compile_fixture(load_limits.SOURCE, root)
        pipe_helper, pipe_compile = compile_fixture(PIPE, root)
        report.update(runtime=str(runtime), load_fixture=load_compile, pipe_fixture=pipe_compile)
        for arguments, language in [(row, row[1]) for row in controllers] + [((row,), row.language) for row in loads]:
            environment = {'LC_ALL': language, 'LANG': language}
            if language != 'C':
                environment.update(locale['environment'])
            if len(arguments) == 4:
                result = controller_run(*arguments, root, runtime, engines, environment, pipe_helper)
            else:
                result = load_run(*arguments, root, runtime, engines, environment, load_helper)
            report['cases'].append(result)
            print('PASS' if result['passed'] else 'FAIL', result['id'],
                  [r['differences'] for r in result['observations']], flush=True)
            if result.get('zero_credit'):
                break
    report['totals'] = {'cases': len(report['cases']), 'pass': sum(row['passed'] for row in report['cases'])}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open('w') as output:
        json.dump(report, output, indent=2)
        output.write('\n')
    return int(report['totals']['cases'] != 14 or report['totals']['pass'] != 14)


if __name__ == '__main__':
    raise SystemExit(main())
