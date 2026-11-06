#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original strict debug-output profiles; exact GNU headers and predetermined paths."""
from __future__ import annotations
import argparse
import base64
from dataclasses import dataclass, field, replace
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
import tomllib
from typing import Any

import conformance as h
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / 'tests/debug-output'
TOKEN = re.compile(b'@[A-Z_]+@')


@dataclass(frozen=True)
class Model:
    id: str
    files: dict[str, bytes]
    args: tuple[str, ...]
    environment: dict[str, str]
    expected: dict[str, Any]
    seen: bytes
    output: str = 'separate'
    updates: dict[str, bytes] = field(default_factory=dict)
    mtimes_ns: dict[str, int] = field(default_factory=dict)
    explicit_version_reference: bytes | None = None
    release_after: bytes | None = None


def safe_relative(name: str) -> Path:
    path = Path(name)
    if not name or path.is_absolute() or '..' in path.parts or '\0' in name:
        raise h.HarnessError('debug fixture path must stay inside its input directory')
    return path


def _model_file(filename: str) -> list[Model]:
    data = tomllib.loads((SOURCES / filename).read_text())
    if data.get('schema_version') != 1:
        raise h.HarnessError('unsupported debug model schema')
    if any(row.get('modules') for row in data['models']):
        raise h.HarnessError('static debug profiles must not compile native modules')
    result = [Model(row['id'], {name: bytes.fromhex(value) for name, value in row['files'].items()},
                    tuple(row['args']), row['environment'], row['expected'],
                    bytes.fromhex(row['expected_seen_template_hex']), row.get('output', 'separate'),
                    {name: bytes.fromhex(value) for name, value in row.get('expected_file_updates', {}).items()},
                    row.get('mtimes_ns', {}),
                    bytes.fromhex(row['oracle_explicit_version_hex']) if 'oracle_explicit_version_hex' in row else None,
                    bytes.fromhex(row['release_after_stdout_hex']) if 'release_after_stdout_hex' in row else None)
              for row in data['models']]
    for model in result:
        stdout = bytes.fromhex(model.expected['stdout_template_hex'])
        stderr = bytes.fromhex(model.expected['stderr_template_hex'])
        if model.release_after is not None and (
            not model.release_after.endswith(b'\n') or stdout.count(model.release_after) != 1
            or {'started', 'started.tmp', 'release', 'release.tmp'} & model.files.keys()
            or model.updates.get('started') != b'started\n' or model.updates.get('release') != b'yes\n'
        ):
            raise h.HarnessError('invalid held-worker release boundary')
        uses_version = b'@EXPLICIT_VERSION@' in stdout
        if uses_version != (model.explicit_version_reference is not None):
            raise h.HarnessError('explicit version reference and slot must agree')
        if b'@EXPLICIT_VERSION@' in stderr or (uses_version and
                (model.id != 'debug-invalid-before-version' or '--version' not in model.args
                 or stdout.count(b'@EXPLICIT_VERSION@') != 1)):
            raise h.HarnessError('explicit version branding is restricted to its one declared boundary')
    if len({model.id for model in result}) != len(result):
        raise h.HarnessError('duplicate debug profile ID')
    return result


def models() -> list[Model]:
    result = (_model_file('models.toml') + _model_file('extension.toml')
              + _model_file('child.toml') + _model_file('vpath.toml')
              + _model_file('dispatch.toml') + _model_file('revisit.toml')
              + _model_file('prerequisite-wait.toml') + _model_file('post-reap.toml')
              + _model_file('wait-reap.toml') + _model_file('grouped-reap.toml')
              + _model_file('notparallel-reap.toml') + stem_limit_models())
    if len({model.id for model in result}) != len(result):
        raise h.HarnessError('duplicate debug profile ID across model files')
    return result


def stem_limit_models() -> list[Model]:
    """Use bounded spellings and no on-disk paths near the Linux limit."""
    header = (
        b'GNU Make 4.4.1\nBuilt for x86_64-pc-linux-gnu\n'
        b'Copyright (C) 1988-2023 Free Software Foundation, Inc.\n'
        b'License GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>\n'
        b'This is free software: you are free to change and redistribute it.\n'
        b'There is NO WARRANTY, to the extent permitted by law.\n'
        b'Reading makefiles...\nUpdating makefiles....\nUpdating goal targets....\n'
    )

    def directory(length: int) -> bytes:
        prefix = (b'd' * 252 + b'/') * 16
        return prefix + b'e' * (length - len(prefix) - 1) + b'/'

    below, exact, above = (directory(length - 5) for length in (4095, 4096, 4097))
    rows = (
        ('short', b'', b'%.out', b'build', True, True),
        ('short-relative', b'virtual/', b'%.out', b'build', True, True),
        ('directory-below', below, b'%.out', b'build', True, True),
        ('directory-exact', exact, b'%.out', b'build', True, True),
        ('directory-above', above, b'%.out', b'build', False, True),
        ('slash-pattern-above', b'', b'head/%.out', above + b'build', False, True),
        ('long-literal-pattern', b'', above + b'%.out', b'build', True, True),
        ('quiet-rejects-before-expansion', above, b'%.out', b'build', False, False),
    )
    models = []
    for suffix, prefix, pattern, stem, selected, trace in rows:
        target = prefix + pattern.replace(b'%', stem)
        args = ('-rR', '--no-print-directory') + (('--debug=i',) if trace else ())
        secondary = b'' if trace else b'.SECONDEXPANSION:\nSIDE = $(info unexpected-expansion)\n'
        prerequisites = b'' if trace else b' $$(SIDE)'
        makefile = secondary + pattern + b':' + prerequisites + b'\n\t@$(info selected-implicit):\n'
        if not trace:
            makefile += b'.DEFAULT:\n\t@$(info selected-default):\n'
        status = 2 if trace and not selected else 0
        stat_error = b'make: stat: %s: File name too long\n' % target if len(target) >= 4096 else b''
        stderr = (stat_error + b"make: *** No rule to make target '%s'.  Stop.\n" % target
                  if status else stat_error * 2)
        stdout = b'selected-implicit\n' if selected else b'selected-default\n'
        if trace:
            label = pattern + b':'
            trying = b"  Trying pattern rule '%s' with stem '%s'.\n" % (label, stem)
            if selected:
                search = trying + b" Found implicit rule '%s' for '%s'.\n" % (label, target)
            else:
                rejected = trying + b"  Stem too long: '%s'.\n" % (prefix + stem)
                search = rejected + b'  Trying harder.\n' + rejected
                search += b" No implicit rule found for '%s'.\n" % target
            completion = stdout + b"Successfully remade target file '%s'.\n" % target if selected else b''
            stdout = (
                header + b" File '%s' does not exist.\n" % target
                + b" Looking for an implicit rule for '%s'.\n" % target + search
                + b"Must remake target '%s'.\n" % target + completion
            )
        models.append(Model('debug-stem-limit-' + suffix,
                            {'Makefile': makefile, 'seen': b'anchor\n'},
                            (*args, os.fsdecode(target)), {},
                            {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                             'stderr_template_hex': stderr.hex()},
                            b'anchor\n'))
    return models


def render(template: bytes, runtime: Path, explicit_version: bytes | None = None) -> bytes:
    """Paths come from the declared reset root, never from child output."""
    values = {b'@WORK@': os.fsencode(runtime / 'work'), b'@BIN@': os.fsencode(runtime / 'bin' / 'make')}
    if explicit_version is not None:
        values[b'@EXPLICIT_VERSION@'] = explicit_version
    def substitute(match: re.Match[bytes]) -> bytes:
        if match[0] not in values:
            raise h.HarnessError('unsupported debug template token: ' + match[0].decode())
        return values[match[0]]
    return TOKEN.sub(substitute, template)


def prepare(model: Model, directory: Path, runtime: Path) -> tuple[h.Case, dict[str, Any]]:
    directory.mkdir(mode=0o755)
    for name, value in model.files.items():
        target = directory / safe_relative(name)
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o755)
        target.write_bytes(render(value, runtime))
        target.chmod(0o644)
    for name, stamp in model.mtimes_ns.items():
        os.utime(directory / safe_relative(name), ns=(stamp, stamp))
    h.validate_fixture(directory)
    arguments = tuple(os.fsdecode(render(os.fsencode(argument), runtime)) for argument in model.args)
    environment = {key: os.fsdecode(render(os.fsencode(value), runtime)) for key, value in model.environment.items()}
    case = h.Case(directory, model.id, 'debug.output', 'required', arguments, 5, environment, {}, model.output)
    filesystem = h.snapshot({'work': directory})
    filesystem.update({name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp')})
    if 'seen' in model.updates and model.updates['seen'] != model.seen:
        raise h.HarnessError('conflicting ordered seen expectation')
    for name, value in {**model.updates, 'seen': model.seen}.items():
        relative = safe_relative(name)
        content = render(value, runtime)
        for parent in relative.parents:
            if parent != Path('.'):
                filesystem['work/' + str(parent)] = {'type': 'directory', 'mode': 0o755}
        key = 'work/' + str(relative)
        filesystem[key] = {'type': 'file', 'mode': filesystem.get(key, {}).get('mode', 0o644),
                           'size': len(content), 'sha256': hashlib.sha256(content).hexdigest(),
                           'content': base64.b64encode(content).decode()}
    return case, filesystem


def evidence(observation: h.Observation) -> dict[str, Any]:
    return dict(h.observation_json(observation), stdout_hex=observation.stdout.hex(), stderr_hex=observation.stderr.hex())


def assess(model: Model, case: h.Case, runtime: Path, filesystem: dict[str, Any],
           observations: list[h.Observation], versions: list[bytes | None] | None = None) -> dict[str, Any]:
    if len(observations) not in (2, 3):
        raise h.HarnessError('debug assessment requires GNU2 and optional candidate')
    versions = versions or [None] * len(observations)
    if len(versions) != len(observations):
        raise h.HarnessError('debug version calibration count must match observations')
    expected_errors = []
    for observation, version in zip(observations, versions):
        expected = {'exit_code': model.expected['exit_code'],
                    'stdout_hex': render(bytes.fromhex(model.expected['stdout_template_hex']), runtime, version).hex(),
                    'stderr_hex': render(bytes.fromhex(model.expected['stderr_template_hex']), runtime).hex()}
        expected_errors.append(h.expectation_errors(replace(case, expected=expected), observation))
    filesystem_errors = [observation.filesystem != filesystem for observation in observations]
    repeat = h.differences(observations[0], observations[1])
    # Whole anchored stream expectations above remain mandatory for each engine.
    # Only this declared explicit --version block permits different branding.
    compared = [replace(o, stdout=b'') for o in observations] if model.explicit_version_reference is not None else observations
    candidate = h.differences(compared[0], compared[2]) if len(observations) == 3 else None
    return {'passed': not any(expected_errors) and not any(filesystem_errors) and not repeat and not candidate,
            'expected_errors': expected_errors, 'filesystem_errors': filesystem_errors,
            'oracle_repeat_differences': repeat, 'candidate_differences': candidate,
            'observations': [evidence(observation) for observation in observations]}


def valid_explicit_version(observation: h.Observation, filesystem: dict[str, Any], reference: bytes | None = None) -> bool:
    forbidden = (b'Entering directory', b'Leaving directory', b'Considering target',
                 b'Reading makefile', b'Updating goal', b'EARLY', b'unknown debug', b'DEBUG_VERSION_READER')
    return (observation.returncode == 0 and not observation.stderr and not observation.limit
            and not observation.leftover_processes and observation.filesystem == filesystem
            and observation.stdout.startswith((b'GNU Make ', b'rmake '))
            and not any(marker in observation.stdout for marker in forbidden)
            and (reference is None or observation.stdout == reference))


def calibrate_version(binary: Path, root: Path, reference: bytes | None = None) -> tuple[bytes, list[dict[str, Any]]]:
    root.mkdir(mode=0o755)
    fixture = root / 'input'
    fixture.mkdir(mode=0o755)
    (fixture / 'Makefile').write_bytes(b'$(error DEBUG_VERSION_READER)\n')
    filesystem = h.snapshot({'work': fixture})
    filesystem.update({name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp')})
    runtime = root / 'runtime'
    runtime.mkdir(mode=0o755)
    case = h.Case(fixture, 'debug-explicit-version', 'debug.output', 'required', ('--version',), 5, {}, {})
    observations = [h.execute(case, runtime, binary) for _ in range(2)]
    records = [evidence(observation) for observation in observations]
    if not all(valid_explicit_version(observation, filesystem, reference) for observation in observations) or h.differences(*observations):
        raise h.HarnessError('invalid isolated explicit-version calibration: ' + json.dumps(records))
    return observations[0].stdout, records


def execute_model(model: Model, case: h.Case, runtime: Path, binary: Path) -> h.Observation:
    boundary = model.release_after
    if boundary is None:
        return h.execute(case, runtime, binary)
    work = runtime / 'work'
    released = False

    def release(_parent: int, stdout: bytes) -> None:
        nonlocal released
        if released or boundary not in stdout or not (work / 'started').is_file():
            return
        if (work / 'started').read_bytes() != b'started\n' or (work / 'done').exists():
            raise h.HarnessError('worker was not held at the declared boundary')
        temporary = work / 'release.tmp'
        temporary.write_bytes(b'yes\n')
        temporary.chmod(0o644)
        temporary.rename(work / 'release')
        released = True

    return h.execute(case, runtime, binary, observe=release)


def run_campaign(selected: list[Model], oracle: Path, candidate: Path | None, root: Path) -> dict[str, Any]:
    inputs = root / 'inputs'
    inputs.mkdir()
    runtime = root / 'runtime'
    runtime.mkdir()
    references = {model.explicit_version_reference for model in selected if model.explicit_version_reference is not None}
    if len(references) > 1:
        raise h.HarnessError('conflicting explicit GNU version references')
    oracle_version, oracle_calibration = calibrate_version(oracle, root / 'oracle-version', references.pop()) if references else (None, [])
    candidate_version, candidate_calibration = calibrate_version(candidate, root / 'candidate-version') if candidate is not None and oracle_version is not None else (None, [])
    records = []
    for model in selected:
        case, filesystem = prepare(model, inputs / safe_relative(model.id), runtime)
        observations = [execute_model(model, case, runtime, binary) for binary in
                        ([oracle, oracle] if candidate is None else [oracle, oracle, candidate])]
        judgment = assess(model, case, runtime, filesystem, observations,
                          [oracle_version, oracle_version] + ([candidate_version] if candidate is not None else []))
        records.append({'id': model.id, 'args': list(case.args), 'environment': case.environment,
                        'expected': model.expected, 'expected_seen_template_hex': model.seen.hex(),
                        'input_files_hex': {name: value.hex() for name, value in model.files.items()},
                        'input_mtimes_ns': model.mtimes_ns, 'expected_filesystem': filesystem,
                        'release_after_stdout_hex': model.release_after.hex() if model.release_after is not None else None,
                        'output': model.output, 'oracle_explicit_version_hex': model.explicit_version_reference.hex() if model.explicit_version_reference is not None else None, **judgment})
        print(('PASS ' if judgment['passed'] else 'FAIL ') + model.id, flush=True)
    return {'classification': 'Original static debug-output oracle qualification; zero ordinary-ledger or exception credit' if candidate is None else 'Original static debug-output differential profile; ordinary-ledger credit is separate',
            'oracle_only': candidate is None, 'passed': all(row['passed'] for row in records),
            'runtime_path': str(runtime), 'explicit_version_calibrations': {'oracle': oracle_calibration, 'candidate': candidate_calibration},
            'totals': {'cases': len(records), 'pass': sum(row['passed'] for row in records), 'fail': sum(not row['passed'] for row in records)},
            'records': records}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/debug-output/campaign.json')
    parser.add_argument('--case', action='append', default=[])
    args = parser.parse_args(argv)
    if args.oracle_only == (args.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    report: dict[str, Any] = {'passed': False}
    try:
        available = models()
        unknown = set(args.case) - {model.id for model in available}
        if unknown:
            raise h.HarnessError('unknown debug cases: ' + ', '.join(sorted(unknown)))
        selected = [model for model in available if not args.case or model.id in args.case]
        reference_oracle.verify(args.oracle)
        identities = {'oracle': h.executable_identity(args.oracle, oracle=True)}
        if args.candidate is not None:
            identities['candidate'] = h.executable_identity(args.candidate)
        with tempfile.TemporaryDirectory(prefix='rmake-debug-output-') as temporary:
            report = run_campaign(selected, Path(identities['oracle']['path']),
                                  Path(identities['candidate']['path']) if args.candidate is not None else None,
                                  Path(temporary).resolve())
        report['identities'] = identities
        report['source_hashes'] = {'driver': h.sha256_file(Path(__file__)), 'models': h.sha256_file(SOURCES / 'models.toml'), 'extension': h.sha256_file(SOURCES / 'extension.toml'), 'child': h.sha256_file(SOURCES / 'child.toml'), 'conformance': h.sha256_file(ROOT / 'tools/conformance.py')}
        status = 0 if report['passed'] else 1
    except (h.HarnessError, reference_oracle.OracleError, OSError, ValueError, KeyError) as error:
        report.update(error=str(error), passed=False)
        print('ERROR ' + str(error), flush=True)
        status = 2
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print('Debug output gate:', report.get('totals', {'error': True}), flush=True)
    return status


if __name__ == '__main__':
    raise SystemExit(main())
