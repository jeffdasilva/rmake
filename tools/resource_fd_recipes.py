# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact GNU comparisons for recipe launch and cancellation at low FD limits."""
from pathlib import Path
import argparse
import json
import os
import shutil
import signal
import tempfile

import conformance as harness
import oracle
import resource_fd as limits

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / 'tests/resource-fd-recipes'
MODELS = FIXTURES / 'models.json'
MAKE_ARGS = ('-rR', '--no-print-directory')


def launch(row, argv):
    return ['/usr/bin/python3', 'launch.py', str(row.get('nofile', 4)),
            row.get('signal', 'default'), 'inherit', *argv]


def prepare(row, inputs, compiler):
    inputs.mkdir()
    (inputs / 'Makefile').write_text(row['makefile'])
    for name, record in row.get('files', {}).items():
        file = inputs / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(record['text'])
        file.chmod(record['mode'])
    shutil.copyfile(FIXTURES / 'launch.py', inputs / 'launch.py')
    compilation = None
    if row.get('compile'):
        command = [compiler, '-std=c11', '-Wall', '-Wextra', '-Werror', '-O2',
                   '-o', str(inputs / 'entry'), str(FIXTURES / 'probe.c')]
        out, err, status, bound, left = harness.capture(
            command, inputs, {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}, 10)
        if status or bound or left:
            raise harness.HarnessError(f'original fixture compilation failed: {status}, {err!r}')
        compilation = {'command': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                       'binary_sha256': harness.sha256_file(inputs / 'entry')}
    harness.validate_fixture(inputs)
    return compilation


def shell_expected(row, case, runtime, reference):
    """Calibrate only the independently executed system shell's FD4 behavior."""
    controls = []
    for _ in range(2):
        harness.reset_workspace(case, runtime, reference)
        roots = {key: runtime / key for key in ('bin', 'home', 'tmp', 'work')}
        before = harness.snapshot(roots)
        command = launch(row, row['shell_control'])
        out, err, status, bound, left = harness.capture(
            command, roots['work'], harness.clean_environment(runtime), case.timeout)
        if bound or left or harness.snapshot(roots) != before or not 0 <= status <= 255:
            raise harness.HarnessError('independent descriptor-limited shell control failed')
        controls.append((out, err, status))
    if controls[0] != controls[1]:
        raise harness.HarnessError('independent descriptor-limited shell did not repeat')
    out, err, status = controls[0]
    evidence = {'argv': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                'status': status, 'repeated_exact': True}
    if status:
        err += f"make: *** [Makefile:{row['recipe_line']}: all] Error {status}\n".encode()
    return (out, err, 2 if status else 0), evidence


def run_recipe(row, root, runtime, binaries, compiler):
    inputs = root / row['id']
    compilation = prepare(row, inputs, compiler)
    case = harness.Case(inputs, row['id'], 'resource.recipe-launch', 'required',
                        MAKE_ARGS, 6.0, {}, {})
    shell_control = None
    if 'shell_control' in row:
        expected_streams, shell_control = shell_expected(row, case, runtime, binaries[0])
    else:
        expected_streams = (row['stdout'].encode(), row['stderr'].encode(), row['status'])
    observations = []
    for binary in binaries:
        harness.reset_workspace(case, runtime, binary)
        roots = {key: runtime / key for key in ('bin', 'home', 'tmp', 'work')}
        expected = harness.snapshot(roots)
        expected['bin/make'] = {'type': 'symlink', 'mode': 0o777, 'target': str(binary)}
        if row.get('touch'):
            expected['work/goal'] = limits.file_record('')
        command = launch(row, [str(runtime / 'bin/make'), *MAKE_ARGS])
        out, err, status, bound, left = harness.capture(
            command, roots['work'], harness.clean_environment(runtime, row.get('environment')),
            case.timeout)
        filesystem = harness.snapshot(roots)
        errors = [name for name, wrong in (
            ('stdout', out != expected_streams[0]), ('stderr', err != expected_streams[1]),
            ('status', status != expected_streams[2]), ('bounds/leftovers', bool(bound or left)),
            ('complete filesystem', filesystem != expected),
        ) if wrong]
        observations.append({'argv': command, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                             'status': status, 'bound': bound, 'leftovers': left,
                             'filesystem': filesystem, 'expected_filesystem': expected, 'errors': errors})
    return comparison(row, observations, compilation=compilation, shell_control=shell_control)


def run_cancellation(row, root, runtime, binaries, controls):
    inputs = root / row['id']
    cell = limits.fixtures.Cell(signal.Signals(row['signal']), 'parent')
    limits.fixtures.materialize(cell, inputs)
    harness.validate_fixture(inputs)
    case = harness.Case(inputs, row['id'], 'resource.cancellation', 'required',
                        tuple(controls['make_args']), 6.0, {}, {})
    observations = [limits.observe(case, runtime, binary, row, controls, index >= 2)
                    for index, binary in enumerate(binaries)]
    return comparison(row, observations)


def comparison(row, observations, **details):
    comparable = ('stdout_hex', 'stderr_hex', 'status', 'bound', 'leftovers', 'filesystem')
    repeated = all(observations[0][key] == observations[1][key] for key in comparable)
    gnu2 = repeated and all(not item['errors'] for item in observations[:2])
    return {'model': row, 'repeat_exact': repeated, 'gnu2_qualified': gnu2,
            'passed': gnu2 and all(not item['errors'] for item in observations),
            'observations': observations, **details}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/resource-fd-recipes/campaign.json')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--cc', default='cc')
    options = parser.parse_args()
    if options.oracle_only == (options.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    reference = options.oracle.resolve()
    profile = oracle.verify(reference)
    data = json.loads(MODELS.read_bytes())
    if data['schema_version'] != 1:
        raise harness.HarnessError('unsupported low descriptor recipe model schema')
    cancellation = data['cancellation']
    models = [(False, row) for row in data['models']]
    models += [(True, row) for row in cancellation['models']]
    names = {row['id'] for _, row in models}
    if not names or len(names) != len(models) or set(options.case) - names:
        raise harness.HarnessError('empty, unknown, or duplicate low descriptor recipe case')
    selected = [(cancel, row) for cancel, row in models
                if not options.case or row['id'] in options.case]
    compiler = shutil.which(options.cc)
    if compiler is None and any(row.get('compile') for _, row in selected):
        raise harness.HarnessError('selected low descriptor recipe fixtures require a C compiler')
    binaries = [reference, reference]
    identities = {'oracle': harness.executable_identity(reference, oracle=True)}
    if options.candidate is not None:
        candidate = options.candidate.resolve()
        identities['candidate'] = harness.executable_identity(candidate)
        binaries.append(candidate)
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-low-fd-recipes-') as temporary:
            root = Path(temporary)
            runtime = root / 'runtime'
            runtime.mkdir()
            records = [
                run_cancellation(row, root, runtime, binaries, cancellation['stable_inputs'])
                if cancel else run_recipe(row, root, runtime, binaries, compiler)
                for cancel, row in selected
            ]
    finally:
        os.umask(previous_umask)
    for row in records:
        print('PASS' if row['passed'] else 'FAIL', row['model']['id'], flush=True)
    totals = {'cases': len(records), 'gnu2': sum(row['gnu2_qualified'] for row in records),
              'pass': sum(row['passed'] for row in records), 'fail': sum(not row['passed'] for row in records)}
    files = {'models': MODELS, 'runner': Path(__file__), 'probe': FIXTURES / 'probe.c',
             'launcher': FIXTURES / 'launch.py', 'capture': Path(harness.__file__),
             'cancellation': Path(limits.__file__), 'signal_fixture': Path(limits.fixtures.__file__)}
    report = {
        'oracle_profile': profile['profile_id'], 'identities': identities, 'passed': not totals['fail'],
        'totals': totals, 'records': records, 'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
        'source_hashes': {name: harness.sha256_file(path) for name, path in files.items()},
    }
    options.report.parent.mkdir(parents=True, exist_ok=True)
    options.report.write_text(json.dumps(report, indent=2) + '\n')
    print('Low descriptor recipe gate', totals, flush=True)
    return int(bool(totals['fail']))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (harness.HarnessError, oracle.OracleError, OSError, KeyError, ValueError) as error:
        raise SystemExit(f'low descriptor recipe gate: {error}') from error
