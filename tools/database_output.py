#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Strict original GNU database-output profiles; -p remains an open feature.

The full reference transcript is checked, including logical hash/cache
statistics. A separate, sealed input plan prepares configured paths and cache
fields from independently captured host inputs. Only declared wall-clock and
physical identity fields use relational checks. No case output supplies inputs
to its own expectation.
"""
from __future__ import annotations

import argparse
import base64
import ctypes
from dataclasses import dataclass, replace
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import tempfile
import time
import tomllib
from typing import Any

import conformance as h
import database_expected as portable
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / 'tests/database-output'
EXPECTED_INPUTS = SOURCES / 'expected-inputs.json'
TOKEN = re.compile(rb'@[A-Z_0-9]+@')
# GNU uses libc time(NULL), whose Linux vdso/coarse domain can lag the
# high-resolution realtime clock across a second boundary. Sample that exact
# domain independently; never compensate with a widened tolerance.
LIBC = ctypes.CDLL('libc.so.6', use_errno=True)
LIBC.time.argtypes = [ctypes.c_void_p]
LIBC.time.restype = ctypes.c_long

CTIME = re.compile(rb'(Mon|Tue|Wed|Thu|Fri|Sat|Sun) (Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) ([ 0-9][0-9]) ([0-9]{2}:[0-9]{2}:[0-9]{2}) ([0-9]{4})')


@dataclass(frozen=True)
class Model:
    id: str
    files: dict[str, bytes]
    args: tuple[str, ...]
    environment: dict[str, str]
    fixed_mtime_ns: int
    expected_status: int
    seen: bytes
    stdout: bytes
    stderr: bytes
    directories: tuple[str, ...]
    required: tuple[bytes, ...]
    forbidden: tuple[bytes, ...]
    sections: tuple[bytes, ...]
    user_variable_count: int | None = None
    dump_count: int = 1
    compile_shared: tuple[tuple[str, str], ...] = ()
    extra_files: tuple[tuple[str, bytes], ...] = ()
    removed_files: tuple[str, ...] = ()


@dataclass(frozen=True)
class Execution:
    observation: h.Observation
    interval: tuple[float, float]
    # These values are captured from predetermined model paths before and after
    # the child runs, independently of all stdout bytes.
    directories_before: dict[str, dict[str, int]]
    directories_after: dict[str, dict[str, int]]
    libc_time_interval: tuple[int, int] | None = None
    expected_inputs: portable.PublicInputs | None = None


def expected_preparation(profile: dict[str, Any], root: Path) -> portable.Preparation:
    return portable.Preparation.load(profile, EXPECTED_INPUTS, root / 'database-input-witness')


def expected_source_hashes() -> dict[str, str]:
    return {name: h.sha256_file(path) for name, path in {
        'expected_input_plans': EXPECTED_INPUTS, 'expected_input_replay': Path(portable.__file__),
        'public_input_witness': ROOT / 'tools/fixtures/database_inputs.c'}.items()}


def prepare_expected_model(model: Model, execution: Execution) -> Model:
    if execution.expected_inputs is None:
        # Literal-only mode is retained for archived observer controls and unit
        # tests. All three current executable drivers provide preparation.
        return model
    return replace(model, stdout=portable.render_model(model.stdout, model.dump_count, execution.expected_inputs))


def libc_time() -> int:
    value = int(LIBC.time(None))
    if value == -1:
        raise h.HarnessError('libc time(NULL) sampling failed')
    return value


def safe_relative(name: str, *, dot: bool = False) -> Path:
    value = Path(name)
    if not name or value.is_absolute() or '..' in value.parts or '\0' in name or (value == Path('.') and not dot):
        raise h.HarnessError('database fixture path must stay inside its declared root')
    return value


def models(suite: str = 'core') -> list[Model]:
    if suite == 'all':
        return models('core') + models('phases') + models('search')
    if suite not in ('core', 'phases', 'search'):
        raise h.HarnessError('unknown database suite')
    data = (tomllib.loads((SOURCES / 'models.toml').read_text()) if suite == 'core'
            else json.loads((SOURCES / ('phases.json' if suite == 'phases' else 'search-spelling.json')).read_text()))
    if data.get('schema_version') != 1:
        raise h.HarnessError('unsupported database model schema')
    result = []
    for row in data['models']:
        model = Model(row['id'], {name: bytes.fromhex(value) for name, value in row['files'].items()},
                      tuple(row['args']), row['environment'], row['fixed_mtime_ns'], row['expected_status'],
                      bytes.fromhex(row['expected_seen_hex']), bytes.fromhex(row['stdout_template_hex']),
                      bytes.fromhex(row['stderr_hex']), tuple(row['directory_slots']),
                      tuple(bytes.fromhex(value) for value in row['required_fragments_hex']),
                      tuple(bytes.fromhex(value) for value in row['forbidden_fragments_hex']),
                      tuple(bytes.fromhex(value) for value in row['expected_sections_hex']),
                      row.get('user_variable_count'), row.get('dump_count', 1),
                      tuple(tuple(pair) for pair in row.get('compile_shared', [])),
                      tuple((name, bytes.fromhex(value)) for name, value in row.get('expected_additions_hex', {}).items()),
                      tuple(row.get('expected_removals', [])))
        safe_relative(model.id)
        for name in model.files:
            safe_relative(name)
        for name in model.directories:
            safe_relative(name, dot=True)
        if not 1 <= model.dump_count <= 4:
            raise h.HarnessError('invalid declared database dump count')
        for source, output in model.compile_shared:
            safe_relative(source); safe_relative(output)
            if source not in model.files or output in model.files:
                raise h.HarnessError('compiled fixture source/output conflict')
        for name, _ in model.extra_files:
            safe_relative(name)
        if len(set(model.directories)) != len(model.directories):
            raise h.HarnessError('database directory slots must name unique predetermined paths')
        # Validate every template token even when selecting a different case.
        compile_template(model, Path('/declared/reset/workspace'))
        result.append(model)
    if len({model.id for model in result}) != len(result):
        raise h.HarnessError('duplicate database profile ID')
    return result


def render_paths(value: bytes, runtime: Path) -> bytes:
    # The absolute reset root is chosen before running any child.
    return value.replace(b'@RUNTIME@', os.fsencode(runtime))


def clock_fields(model: Model) -> tuple[str, ...]:
    if b'@CTIME_OPEN@' in model.stdout or b'@CTIME_CLOSE@' in model.stdout:
        if model.dump_count != 1:
            raise h.HarnessError('legacy clock slots declare one dump only')
        return ('CTIME_OPEN', 'CTIME_CLOSE')
    return tuple(f'CTIME_{kind}_{index}' for index in range(model.dump_count) for kind in ('OPEN', 'CLOSE'))


def compile_template(model: Model, runtime: Path) -> re.Pattern[bytes]:
    pieces = [b'\\A']
    last = 0
    counts: dict[bytes, int] = {}
    clocks = {f'@{name}@'.encode() for name in clock_fields(model)}
    for match in TOKEN.finditer(model.stdout):
        token = match[0]
        counts[token] = counts.get(token, 0) + 1
        pieces.append(re.escape(model.stdout[last:match.start()]))
        if token == b'@RUNTIME@':
            pieces.append(re.escape(os.fsencode(runtime)))
        elif token in clocks:
            if counts[token] != 1:
                raise h.HarnessError('database clock slot repeated')
            pieces.append(b'(?P<' + token[1:-1] + b'>[^\\n]{24})')
        else:
            found = re.fullmatch(rb'@(DEVICE|INODE)_([0-9]+)@', token)
            if found is None or int(found[2]) >= len(model.directories):
                raise h.HarnessError('unknown database template token: ' + repr(token))
            if counts[token] == 1:
                pieces.append(b'(?P<' + token[1:-1] + b'>[0-9]{1,20})')
            else:
                pieces.append(b'(?P=' + token[1:-1] + b')')
        last = match.end()
    pieces.append(re.escape(model.stdout[last:]))
    pieces.append(b'\\Z')
    if any(counts.get(token) != 1 for token in clocks):
        raise h.HarnessError('database clock slots must occur exactly once at declared positions')
    for index in range(len(model.directories)):
        for kind in ('DEVICE', 'INODE'):
            count = counts.get(f'@{kind}_{index}@'.encode(), 0)
            if not 1 <= count <= model.dump_count:
                raise h.HarnessError('physical database slot count differs from declared dumps')
    return re.compile(b''.join(pieces))


def parse_clock(value: bytes) -> float:
    if CTIME.fullmatch(value) is None:
        raise ValueError('ctime must use the exact C-locale protocol')
    parsed = datetime.datetime.strptime(value.decode('ascii'), '%a %b %d %H:%M:%S %Y').replace(tzinfo=datetime.timezone.utc)
    exact = (parsed.strftime('%a %b ') + f'{parsed.day:2} ' + parsed.strftime('%H:%M:%S %Y')).encode('ascii')
    if exact != value:
        raise ValueError('ctime weekday, padding, or calendar value is inconsistent')
    return parsed.timestamp()


def stream_errors(model: Model, runtime: Path, execution: Execution) -> list[str]:
    return prepared_stream_errors(prepare_expected_model(model, execution), runtime, execution)


def prepared_stream_errors(model: Model, runtime: Path, execution: Execution) -> list[str]:
    observation = execution.observation
    match = compile_template(model, runtime).fullmatch(observation.stdout)
    errors = []
    if match is None:
        errors.append('full stdout protocol (including exact independently prepared configuration/cache fields)')
    else:
        clocks = []
        for field in clock_fields(model):
            try:
                stamp = parse_clock(match[field])
                if execution.libc_time_interval is None:
                    errors.append(field + ' lacks independently captured libc time(NULL) samples')
                elif not execution.libc_time_interval[0] <= stamp <= execution.libc_time_interval[1]:
                    errors.append(field + ' outside independently captured libc time(NULL) interval')
                clocks.append(stamp)
            except (ValueError, UnicodeError, OverflowError):
                errors.append(field + ' is not an exact C-locale ctime')
        if clocks != sorted(clocks):
            errors.append('database closing clock precedes opening clock')
        for index, name in enumerate(model.directories):
            before = execution.directories_before.get(name)
            after = execution.directories_after.get(name)
            if before is None or after is None or before != after or not stat.S_ISDIR(before['mode']):
                errors.append('declared physical directory changed or is absent: ' + name)
                continue
            if int(match[f'DEVICE_{index}']) != before['device'] or int(match[f'INODE_{index}']) != before['inode']:
                errors.append('physical directory identity differs from independent stat: ' + name)
    if observation.stderr != model.stderr:
        errors.append('exact stderr protocol')
    if observation.returncode != model.expected_status:
        errors.append('exit status')
    if observation.limit is not None:
        errors.append('execution bound: ' + observation.limit)
    if observation.leftover_processes:
        errors.append('child cleanup')
    # These pre-observation assertions are retained in addition to the exact
    # full transcript; they do not substitute for any literal protocol bytes.
    for required in model.required:
        if render_paths(required, runtime) not in observation.stdout:
            errors.append('original required model assertion: ' + repr(required))
    for forbidden in model.forbidden:
        if render_paths(forbidden, runtime) in observation.stdout:
            errors.append('original forbidden model assertion: ' + repr(forbidden))
    previous = -1
    for section in model.sections:
        found = observation.stdout.find(section, previous + 1)
        if found < 0:
            errors.append('original section order assertion: ' + repr(section))
        else:
            previous = found
    if model.user_variable_count is not None and len(re.findall(rb'(?m)^DBV_[0-9]{4} := value-[0-9]+$', observation.stdout)) != model.user_variable_count:
        errors.append('original user-variable population assertion')
    return errors


def prepare(model: Model, directory: Path, runtime: Path, compilations: list[dict[str, Any]] | None = None) -> tuple[h.Case, dict[str, Any]]:
    directory.mkdir(mode=0o755)
    for name, content in model.files.items():
        target = directory / safe_relative(name)
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        target.write_bytes(render_paths(content, runtime))
        target.chmod(0o644)
        os.utime(target, ns=(model.fixed_mtime_ns, model.fixed_mtime_ns))
    for source, output in model.compile_shared:
        compiler = shutil.which('cc')
        if compiler is None:
            raise h.HarnessError('trusted original native fixture requires cc')
        target = directory / safe_relative(output)
        target.parent.mkdir(parents=True, exist_ok=True)
        command = [compiler, '-std=c11', '-Wall', '-Wextra', '-Werror', '-g0', '-O2', '-fPIC', '-shared',
                   str(directory / source), '-o', str(target)]
        with tempfile.TemporaryDirectory(prefix='rmake-database-compiler-') as temporary:
            compile_root = Path(temporary)
            (compile_root / 'home').mkdir()
            (compile_root / 'tmp').mkdir()
            out, err, status, limit, leftover = h.capture(command, compile_root, h.clean_environment(compile_root), 30)
        if status != 0 or limit is not None or leftover:
            raise h.HarnessError('original database native fixture failed compilation: ' + repr(err))
        target.chmod(0o644)
        os.utime(target, ns=(model.fixed_mtime_ns, model.fixed_mtime_ns))
        if compilations is not None:
            compilations.append({'command': command, 'source': source, 'output': output,
                                 'source_sha256': h.sha256_file(directory / source),
                                 'output_sha256': h.sha256_file(target),
                                 'stdout_hex': out.hex(), 'stderr_hex': err.hex()})
    h.validate_fixture(directory)
    case = h.Case(directory, model.id, 'database.output', 'required', model.args, 8, model.environment, {})
    filesystem = h.snapshot({'work': directory})
    filesystem.update({name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp')})
    filesystem['work/seen'] = {'type': 'file', 'mode': 0o644, 'size': len(model.seen),
                               'sha256': hashlib.sha256(model.seen).hexdigest(), 'content': base64.b64encode(model.seen).decode()}
    for name in model.removed_files:
        safe_relative(name)
        key = 'work/' + name
        if key not in filesystem or filesystem[key]['type'] != 'file':
            raise h.HarnessError('expected removal must name one declared regular input file')
        del filesystem[key]
    for name, content in model.extra_files:
        content = render_paths(content, runtime)
        filesystem['work/' + name] = {'type': 'file', 'mode': 0o644, 'size': len(content),
                                      'sha256': hashlib.sha256(content).hexdigest(),
                                      'content': base64.b64encode(content).decode()}
    return case, filesystem


def directory_metadata(model: Model, runtime: Path) -> dict[str, dict[str, int]]:
    result = {}
    for name in model.directories:
        # Neither path selection nor identity values use stdout.
        metadata = (runtime / 'work' / safe_relative(name, dot=True)).stat()
        result[name] = {'device': metadata.st_dev, 'inode': metadata.st_ino, 'mode': metadata.st_mode}
    return result


def execute(model: Model, case: h.Case, runtime: Path, executable: Path, *,
            expected: portable.Preparation | None = None) -> Execution:
    h.reset_workspace(case, runtime, executable)
    before = directory_metadata(model, runtime)
    plan = expected.plan(model.id, model.stdout, model.dump_count) if expected is not None else None
    public_before = expected.capture(runtime / "work", plan) if expected is not None and plan is not None else {}
    started = time.time()
    libc_started = libc_time()
    stdout, stderr, status, limit, leftovers = h.capture(
        [str(runtime / 'bin' / 'make'), *case.args], runtime / 'work', h.clean_environment(runtime, case.environment), case.timeout)
    libc_ended = libc_time()
    ended = time.time()
    after = directory_metadata(model, runtime)
    public_after = expected.capture(runtime / "work", plan) if expected is not None and plan is not None else {}
    public = (portable.PublicInputs(expected.config, public_before, public_after, expected.witness, plan)
              if expected is not None and plan is not None else None)
    filesystem = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    return Execution(h.Observation(stdout, stderr, status, limit, filesystem, leftovers), (started, ended), before, after, (libc_started, libc_ended), public)


def evidence(execution: Execution) -> dict[str, Any]:
    observation = execution.observation
    return dict(h.observation_json(observation), stdout_hex=observation.stdout.hex(), stderr_hex=observation.stderr.hex(),
                execution_interval=list(execution.interval), directories_before=execution.directories_before,
                directories_after=execution.directories_after, libc_time_interval=list(execution.libc_time_interval) if execution.libc_time_interval is not None else None)


def assess(model: Model, runtime: Path, filesystem: dict[str, Any], executions: list[Execution]) -> dict[str, Any]:
    if len(executions) not in (2, 3):
        raise h.HarnessError('database assessment requires GNU2 and optional candidate')
    expected_errors = []
    observations = []
    for execution in executions:
        record = evidence(execution)
        if execution.expected_inputs is not None:
            record['independent_expected_inputs'] = execution.expected_inputs.evidence()
        observations.append(record)
        try:
            prepared = prepare_expected_model(model, execution)
            if execution.expected_inputs is not None:
                record['prepared_stdout_template_hex'] = prepared.stdout.hex()
            errors = prepared_stream_errors(prepared, runtime, execution)
        except (h.HarnessError, ValueError, KeyError) as error:
            errors = ['independent expected input preparation: ' + str(error)]
        expected_errors.append(errors)
    filesystem_errors = [execution.observation.filesystem != filesystem for execution in executions]
    # Every engine must first match the whole anchored protocol and independent
    # physical/time relations. The remaining fields compare without exemptions.
    stable = [replace(execution.observation, stdout=model.stdout) for execution in executions]
    repeat = h.differences(stable[0], stable[1])
    candidate = h.differences(stable[0], stable[2]) if len(stable) == 3 else None
    if expected_errors[0] or expected_errors[1]:
        repeat.append('independent-literal-or-relational-model')
    if candidate is not None and (expected_errors[0] or expected_errors[2]):
        candidate.append('independent-literal-or-relational-model')
    return {'passed': not any(expected_errors) and not any(filesystem_errors) and not repeat and not candidate,
            'expected_errors': expected_errors, 'filesystem_errors': filesystem_errors,
            'oracle_repeat_differences': repeat, 'candidate_differences': candidate,
            'observations': observations}


def run_campaign(selected: list[Model], oracle: Path, candidate: Path | None, root: Path, *,
                 expected: portable.Preparation | None = None) -> dict[str, Any]:
    inputs = root / 'inputs'
    inputs.mkdir()
    runtime = root / 'runtime'
    runtime.mkdir()
    records = []
    for model in selected:
        compilations: list[dict[str, Any]] = []
        case, filesystem = (prepare(model, inputs / safe_relative(model.id), runtime, compilations)
                            if model.compile_shared else prepare(model, inputs / safe_relative(model.id), runtime))
        executions = [execute(model, case, runtime, binary, expected=expected) for binary in ([oracle, oracle] if candidate is None else [oracle, oracle, candidate])]
        judgment = assess(model, runtime, filesystem, executions)
        records.append({'id': model.id, 'args': list(case.args), 'environment': case.environment,
                        'expected_status': model.expected_status, 'stdout_template_hex': model.stdout.hex(),
                        'stderr_hex': model.stderr.hex(), 'expected_seen_hex': model.seen.hex(),
                        'directory_slots': list(model.directories), 'input_files_hex': {name: value.hex() for name, value in model.files.items()},
                        'fixed_mtime_ns': model.fixed_mtime_ns, 'expected_filesystem': filesystem,
                        'declared_dump_count': model.dump_count, 'compilations': compilations, **judgment})
        print(('PASS ' if judgment['passed'] else 'FAIL ') + model.id, flush=True)
    return {'classification': 'Original database protocol qualification; no candidate or ordinary-ledger credit' if candidate is None else 'Original database differential protocol; incomplete feature remains unsupported until all selected models pass',
            'oracle_only': candidate is None, 'passed': all(row['passed'] for row in records), 'runtime_path': str(runtime),
            'functional_cpu_affinity': {'driver': sorted(os.sched_getaffinity(0)), 'child_policy': 'Inherited from driver; h.capture does not change affinity. This is functional containment, not elapsed performance evidence.'},
            'limitations': ['Pinned C locale, UTC, Linux/x86-64 GNU4.4.1 configuration; no broad nondeterminism exemptions',
                            'Physical directory identities checked against declared paths before and after each execution',
                            'Only database opening/closing ctime varies within independently sampled libc time(NULL) bounds; high-resolution realtime retained separately and no widened tolerance applied',
                            'Full transcript snapshots are derived from retained GNU2 observations; original independent assertions are also mandatory'],
            'totals': {'cases': len(records), 'pass': sum(row['passed'] for row in records), 'fail': sum(not row['passed'] for row in records)}, 'records': records}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/database-output/campaign.json')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--suite', choices=('core', 'phases', 'search', 'all'), default='core')
    args = parser.parse_args(argv)
    if args.oracle_only == (args.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    report: dict[str, Any] = {'passed': False}
    try:
        available = models(args.suite)
        unknown = set(args.case) - {model.id for model in available}
        if unknown:
            raise h.HarnessError('unknown database cases: ' + ', '.join(sorted(unknown)))
        selected = [model for model in available if not args.case or model.id in args.case]
        profile = reference_oracle.verify(args.oracle)
        identities = {'oracle': h.executable_identity(args.oracle, oracle=True)}
        if args.candidate is not None:
            identities['candidate'] = h.executable_identity(args.candidate)
        with tempfile.TemporaryDirectory(prefix='rmake-database-output-') as temporary:
            root = Path(temporary).resolve()
            report = run_campaign(selected, Path(identities['oracle']['path']),
                                  Path(identities['candidate']['path']) if args.candidate is not None else None, root,
                                  expected=expected_preparation(profile, root))
        report['identities'] = identities
        report['source_hashes'] = {name: h.sha256_file(path) for name, path in {
            'driver': Path(__file__), 'models': SOURCES / 'models.toml', 'phases': SOURCES / 'phases.json',
            'search': SOURCES / 'search-spelling.json', 'conformance': ROOT / 'tools/conformance.py'}.items()}
        report['source_hashes'].update(expected_source_hashes())
        status = 0 if report['passed'] else 1
    except (h.HarnessError, reference_oracle.OracleError, OSError, ValueError, KeyError) as error:
        report.update(error=str(error), passed=False)
        print('ERROR ' + str(error), flush=True)
        status = 2
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print('Database output gate:', report.get('totals', {'error': True}), flush=True)
    return status


if __name__ == '__main__':
    raise SystemExit(main())
