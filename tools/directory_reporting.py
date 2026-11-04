#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Strict original directory-reporting profiles with predetermined path templates."""
from __future__ import annotations
import argparse
import base64
from dataclasses import dataclass, field, replace
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile
import tomllib
from typing import Any

import conformance as h
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / 'tests/directory-reporting'
TOKENS = (b'@WORK@', b'@HOME@', b'@TMP@', b'@HELP@', b'@VERSION@')


@dataclass(frozen=True)
class Module:
    source: str
    filename: str


@dataclass(frozen=True)
class Model:
    id: str
    files: dict[str, bytes]
    args: tuple[str, ...]
    environment: dict[str, str]
    expected: dict[str, Any]
    seen: bytes
    output: str = 'separate'
    modules: tuple[Module, ...] = ()
    updates: dict[str, bytes] = field(default_factory=dict)


def safe_relative(name: str) -> Path:
    path = Path(name)
    if not name or path.is_absolute() or '..' in path.parts or '\0' in name:
        raise h.HarnessError('directory profile path must stay inside its input directory')
    return path


def models() -> list[Model]:
    data = tomllib.loads((SOURCES / 'models.toml').read_text())
    if data.get('schema_version') != 1:
        raise h.HarnessError('unsupported directory model schema')
    result = [Model(row['id'], {name: bytes.fromhex(value) for name, value in row['files'].items()},
                    tuple(row['args']), row['environment'], row['expected'],
                    bytes.fromhex(row['expected_seen_template_hex']), row.get('output', 'separate'),
                    tuple(Module(**module) for module in row.get('modules', [])),
                    {name: bytes.fromhex(value) for name, value in row.get('expected_file_updates', {}).items()})
              for row in data['models']]
    if len({model.id for model in result}) != len(result):
        raise h.HarnessError('duplicate directory profile ID')
    return result


def render(template: bytes, runtime: Path, banners: dict[bytes, bytes] | None = None) -> bytes:
    """Substitute declared input tokens, never bytes inferred from an observation."""
    values = {b'@WORK@': os.fsencode(runtime / 'work'), b'@HOME@': os.fsencode(runtime / 'home'),
              b'@TMP@': os.fsencode(runtime / 'tmp'), **(banners or {})}
    # Split the original template so replacement bytes cannot become new tokens.
    import re
    parts = re.split(b'(@WORK@|@HOME@|@TMP@|@HELP@|@VERSION@)', template)
    for index in range(1, len(parts), 2):
        if parts[index] not in values:
            raise h.HarnessError('missing declared cosmetic banner')
        parts[index] = values[parts[index]]
    return b''.join(parts)


def prepare(model: Model, directory: Path, runtime: Path, compiler: str | None) -> tuple[h.Case, list[dict[str, Any]], dict[str, Any]]:
    directory.mkdir(mode=0o755)
    for name, value in model.files.items():
        target = directory / safe_relative(name)
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        target.write_bytes(render(value, runtime))
        target.chmod(0o644)
    compilation = []
    for index, module in enumerate(model.modules):
        if compiler is None:
            raise h.HarnessError('compiler required by selected native finalizer profile')
        source = SOURCES / safe_relative(module.source)
        copied = directory / f'module-{index}.c'
        copied.write_bytes(source.read_bytes())
        copied.chmod(0o644)
        target = directory / safe_relative(module.filename)
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        command = [compiler, '-std=c11', '-Wall', '-Wextra', '-Werror', '-fPIC', '-shared', str(copied), '-o', str(target)]
        out, err, status, limit, leftovers = h.capture(command, directory,
            {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'LANG': 'C', 'TZ': 'UTC'}, 15)
        record = {'command': command, 'source': module.source, 'source_sha256': h.sha256_file(source),
                  'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'exit_code': status,
                  'limit': limit, 'leftover_processes': leftovers}
        compilation.append(record)
        if status or limit or leftovers:
            raise h.HarnessError('directory finalizer compilation failed: ' + json.dumps(record))
        target.chmod(0o755)
        record['module_sha256'] = h.sha256_file(target)
    h.validate_fixture(directory)
    arguments = tuple(os.fsdecode(render(os.fsencode(argument), runtime)) for argument in model.args)
    environment = {key: os.fsdecode(render(os.fsencode(value), runtime)) for key, value in model.environment.items()}
    case = h.Case(directory, model.id, 'directory.reporting', 'required', arguments, 5, environment, {}, model.output)
    expected_filesystem = h.snapshot({'work': directory})
    expected_filesystem.update({name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp')})
    if 'seen' in model.updates and model.updates['seen'] != model.seen:
        raise h.HarnessError('conflicting ordered seen expectation')
    for name, value in {**model.updates, 'seen': model.seen}.items():
        relative = safe_relative(name)
        content = render(value, runtime)
        for parent in relative.parents:
            if parent != Path('.'):
                expected_filesystem['work/' + str(parent)] = {'type': 'directory', 'mode': 0o755}
        key = 'work/' + str(relative)
        mode = expected_filesystem.get(key, {}).get('mode', 0o644)
        expected_filesystem[key] = {'type': 'file', 'mode': mode, 'size': len(content),
                                   'sha256': hashlib.sha256(content).hexdigest(),
                                   'content': base64.b64encode(content).decode()}
    return case, compilation, expected_filesystem


def valid_banner(kind: bytes, observation: h.Observation, expected_filesystem: dict[str, Any]) -> bool:
    header_valid = (observation.stdout.startswith(b'Usage: ') if kind == b'@HELP@'
                    else observation.stdout.startswith((b'GNU Make ', b'rmake ')))
    return (observation.returncode == 0 and not observation.stderr and not observation.limit
            and not observation.leftover_processes and observation.filesystem == expected_filesystem
            and bool(observation.stdout) and header_valid
            and b'Entering directory' not in observation.stdout and b'Leaving directory' not in observation.stdout
            and b'DIRECTORY_BANNER_READER' not in observation.stdout)


def calibrate(binary: Path, root: Path, kinds: set[bytes]) -> tuple[dict[bytes, bytes], list[dict[str, Any]]]:
    root.mkdir(mode=0o755)
    fixture = root / 'input'
    fixture.mkdir(mode=0o755)
    (fixture / 'Makefile').write_bytes(b'$(error DIRECTORY_BANNER_READER)\n')
    expected_filesystem = h.snapshot({'work': fixture})
    expected_filesystem.update({name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp')})
    runtime = root / 'runtime'
    runtime.mkdir(mode=0o755)
    banners = {}
    records = []
    for kind in sorted(kinds):
        case = h.Case(fixture, 'directory-banner', 'directory.reporting', 'required',
                      ('--help' if kind == b'@HELP@' else '--version',), 5, {}, {})
        observations = [h.execute(case, runtime, binary) for _ in range(2)]
        if not all(valid_banner(kind, observation, expected_filesystem) for observation in observations) or h.differences(*observations):
            raise h.HarnessError('invalid or unstable isolated directory cosmetic banner')
        banners[kind] = observations[0].stdout
        records.append({'token': kind.decode(), 'observations': [evidence(o) for o in observations]})
    return banners, records


def evidence(observation: h.Observation) -> dict[str, Any]:
    return dict(h.observation_json(observation), stdout_hex=observation.stdout.hex(), stderr_hex=observation.stderr.hex())


def assess(model: Model, case: h.Case, runtime: Path, expected_filesystem: dict[str, Any],
           observations: list[h.Observation], banners: list[dict[bytes, bytes]]) -> dict[str, Any]:
    if len(observations) not in (2, 3) or len(banners) != len(observations):
        raise h.HarnessError('directory assessment requires GNU2 and optional candidate')
    expected_errors = []
    filesystem_errors = []
    for observation, selected in zip(observations, banners):
        expected = {'exit_code': model.expected['exit_code'],
                    'stdout_hex': render(bytes.fromhex(model.expected['stdout_template_hex']), runtime, selected).hex(),
                    'stderr_hex': render(bytes.fromhex(model.expected['stderr_template_hex']), runtime, selected).hex()}
        expected_errors.append(h.expectation_errors(replace(case, expected=expected), observation))
        filesystem_errors.append(observation.filesystem != expected_filesystem)
    repeat = h.differences(observations[0], observations[1])
    cosmetic = any(token in bytes.fromhex(model.expected[key]) for token in (b'@HELP@', b'@VERSION@')
                   for key in ('stdout_template_hex', 'stderr_template_hex'))
    compared = [replace(o, stdout=b'', stderr=b'') for o in observations] if cosmetic else observations
    candidate = h.differences(compared[0], compared[2]) if len(observations) == 3 else None
    passed = not any(expected_errors) and not any(filesystem_errors) and not repeat and not candidate
    return {'passed': passed, 'expected_errors': expected_errors, 'filesystem_errors': filesystem_errors,
            'oracle_repeat_differences': repeat, 'candidate_differences': candidate,
            'observations': [evidence(o) for o in observations]}


def run_campaign(selected: list[Model], oracle: Path, candidate: Path | None,
                 compiler: str | None, root: Path) -> dict[str, Any]:
    inputs = root / 'inputs'
    inputs.mkdir()
    runtime = root / 'runtime'
    runtime.mkdir()
    kinds = {token for model in selected for token in (b'@HELP@', b'@VERSION@')
             if any(token in bytes.fromhex(model.expected[key]) for key in ('stdout_template_hex', 'stderr_template_hex'))}
    oracle_banners, oracle_calibration = calibrate(oracle, root / 'oracle-banners', kinds) if kinds else ({}, [])
    candidate_banners, candidate_calibration = calibrate(candidate, root / 'candidate-banners', kinds) if candidate is not None and kinds else ({}, [])
    rows = []
    for model in selected:
        case, compilation, filesystem = prepare(model, inputs / safe_relative(model.id), runtime, compiler)
        observations = [h.execute(case, runtime, binary) for binary in
                        ([oracle, oracle] if candidate is None else [oracle, oracle, candidate])]
        judgment = assess(model, case, runtime, filesystem, observations,
                          [oracle_banners, oracle_banners] + ([candidate_banners] if candidate is not None else []))
        rows.append({'id': model.id, 'args': list(case.args), 'environment': case.environment,
                     'expected': model.expected, 'expected_seen_template_hex': model.seen.hex(),
                     'input_files_hex': {name: value.hex() for name, value in model.files.items()},
                     'expected_filesystem': filesystem, 'output': model.output,
                     'compilation': compilation, **judgment})
        print(('PASS ' if judgment['passed'] else 'FAIL ') + model.id, flush=True)
    return {'classification': 'Original directory reporting oracle qualification; zero ordinary-ledger or exception credit' if candidate is None else 'Original directory reporting differential profile; ordinary-ledger credit is separate',
            'oracle_only': candidate is None, 'passed': all(row['passed'] for row in rows),
            'runtime_path': str(runtime), 'calibration': {'oracle': oracle_calibration, 'candidate': candidate_calibration},
            'totals': {'cases': len(rows), 'pass': sum(row['passed'] for row in rows), 'fail': sum(not row['passed'] for row in rows)},
            'records': rows}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/directory-reporting/campaign.json')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--cc', default='cc')
    args = parser.parse_args(argv)
    if args.oracle_only == (args.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    report: dict[str, Any] = {'passed': False}
    try:
        available = models()
        unknown = set(args.case) - {model.id for model in available}
        if unknown:
            raise h.HarnessError('unknown directory cases: ' + ', '.join(sorted(unknown)))
        selected = [model for model in available if not args.case or model.id in args.case]
        compiler = shutil.which(args.cc) if any(model.modules for model in selected) else None
        if any(model.modules for model in selected) and compiler is None:
            raise h.HarnessError('C compiler executable not found: ' + args.cc)
        reference_oracle.verify(args.oracle)
        identities = {'oracle': h.executable_identity(args.oracle, oracle=True)}
        if args.candidate is not None:
            identities['candidate'] = h.executable_identity(args.candidate)
        with tempfile.TemporaryDirectory(prefix='rmake-directory-reporting-') as temporary:
            report = run_campaign(selected, Path(identities['oracle']['path']),
                                  Path(identities['candidate']['path']) if args.candidate is not None else None,
                                  compiler, Path(temporary).resolve())
        report['identities'] = identities
        report['source_hashes'] = {'driver': h.sha256_file(Path(__file__)), 'models': h.sha256_file(SOURCES / 'models.toml')}
        status = 0 if report['passed'] else 1
    except (h.HarnessError, reference_oracle.OracleError, OSError, ValueError, KeyError) as error:
        report.update(error=str(error), passed=False)
        print('ERROR ' + str(error), flush=True)
        status = 2
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print('Directory reporting gate:', report.get('totals', {'error': True}), flush=True)
    return status


if __name__ == '__main__':
    raise SystemExit(main())
