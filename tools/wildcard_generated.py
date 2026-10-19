#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original seeded ASCII filename queries with independent fnmatch anchors.

The generated scope excludes escapes, archives, locale-specific matching and
external mutations; curated fixtures and the Linux interposer cover those
boundaries separately. Both engines must satisfy full authored observations.
"""
from __future__ import annotations
import argparse
import base64
import dataclasses
import fnmatch
import hashlib
import json
from pathlib import Path
import random
import sys
import tempfile

import conformance
import oracle

VERSION = 1
PATTERNS = ('*', '*.c', '*.h', 'a*', '*a*', '?.c', '[ab]*', '[!a]*', '.*',
            'src/*', 'src/*.c', 'src/.*', 'd?/*', 'd?/*.h', '[ds]*/*.c', 'missing*')
ARGS = ('-rR', '--no-print-directory')


def match(pattern: str, files: dict[str, bytes]) -> list[str]:
    dirs = {str(Path(name).parent) for name in files if '/' in name}
    names = set(files) | dirs | {'.', '..'} | {directory + '/' + leaf for directory in dirs for leaf in ('.', '..')}
    chunks = pattern.split('/')
    return sorted(name for name in names if len(name.split('/')) == len(chunks)
                  and all((not leaf.startswith('.') or token.startswith('.')) and fnmatch.fnmatchcase(leaf, token)
                          for leaf, token in zip(name.split('/'), chunks)))


def query(patterns: str, files: dict[str, bytes]) -> bytes:
    return ' '.join(name for pattern in patterns.split() for name in match(pattern, files)).encode()


@dataclasses.dataclass(frozen=True)
class Program:
    seed: int
    index: int
    files: dict[str, bytes]
    makefile: bytes
    stdout: bytes
    patterns: tuple[str, ...]


def generate(seed: int, index: int, expressions: int = 32) -> Program:
    rng = random.Random((seed << 32) ^ index)
    files = {'protected.txt': b'Original generated wildcard anchor.\n', '.hidden': b'hidden\n'}
    for directory in ('', 'src/', 'd0/', 'd1/'):
        files[directory + '.hidden'] = b'hidden\n'
        for number in range(12):
            name = ''.join(rng.choices('abcdxyz', k=rng.randint(1, 5))) + str(number)
            files[directory + name + rng.choice(('.c', '.h', '.txt'))] = f'{number}\n'.encode()
    patterns = tuple(' '.join(rng.choices(PATTERNS, k=rng.randint(1, 3))) for _ in range(expressions))
    source = b'# SPDX-License-Identifier: GPL-3.0-or-later\n# Copyright 2026 Jeff DaSilva\n'
    source += b''.join(f'$(info q{i}=[$(wildcard {pattern})])\n'.encode() for i, pattern in enumerate(patterns))
    source += b'$(file >created.txt,generated)\n'
    source += b'$(info after=[$(wildcard *.txt src/*.c *.txt)])\nall:;@:\n'
    files['Makefile'] = source
    stdout = b''.join(f'q{i}=['.encode() + query(pattern, files) + b']\n' for i, pattern in enumerate(patterns))
    final = {**files, 'created.txt': b'generated\n'}
    stdout += b'after=[' + query('*.txt src/*.c *.txt', final) + b']\n'
    return Program(seed, index, files, source, stdout, patterns)


def regular(data: bytes) -> dict:
    return {'type': 'file', 'mode': 0o644, 'size': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
            'content': base64.b64encode(data).decode()}


def expected(program: Program) -> conformance.Observation:
    entries = {name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp', 'work')}
    for name, data in {**program.files, 'created.txt': b'generated\n'}.items():
        if '/' in name:
            entries['work/' + str(Path(name).parent)] = {'type': 'directory', 'mode': 0o755}
        entries['work/' + name] = regular(data)
    return conformance.Observation(program.stdout, b'', 0, None, entries)


def run(programs, reference: Path, candidate: Path, artifacts: Path, identities: dict, total=None) -> int:
    total = len(programs) if total is None else total
    failed = 0
    records = []
    with tempfile.TemporaryDirectory(prefix='rmake-wildcard-generated-') as temporary:
        root = Path(temporary)
        workspace = root / 'execution'
        workspace.mkdir()
        for program in programs:
            case_id = f'wildcard-{program.seed}-{program.index}'
            inputs = root / 'inputs' / case_id
            inputs.mkdir(parents=True)
            for name, data in program.files.items():
                path = inputs / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
                path.chmod(0o644)
            case = conformance.Case(inputs, case_id, 'functions.filesystem', 'required', ARGS, 5, {}, {})
            anchor = expected(program)
            observed = {label: conformance.execute(case, workspace, binary)
                        for label, binary in (('oracle', reference), ('candidate', candidate))}
            errors = [label + ': authored ' + error for label, result in observed.items()
                      for error in conformance.differences(anchor, result)]
            errors += conformance.differences(observed['oracle'], observed['candidate'])
            record = {'seed': program.seed, 'index': program.index, 'patterns': program.patterns,
                      'makefile_sha256': hashlib.sha256(program.makefile).hexdigest(),
                      'expected_stdout_sha256': hashlib.sha256(program.stdout).hexdigest(),
                      'expected_stdout_bytes': len(program.stdout),
                      'replay': {'seed': program.seed, 'start_index': program.index,
                                 'cases': 1, 'expressions': len(program.patterns)},
                      'status': 'FAIL' if errors else 'PASS'}
            if errors:
                failed += 1
                artifact = conformance.save_artifact(artifacts, case, workspace, identities,
                    observed['oracle'], observed['candidate'], errors, 'FAIL')
                record.update(errors=errors, artifact=str(artifact))
            records.append(record)
            if len(records) % 25 == 0 or errors:
                print(f'Wildcard progress {len(records)}/{total}; failed={failed}', flush=True)
            if failed >= 20:
                break
    artifacts.mkdir(parents=True, exist_ok=True)
    report = {'version': VERSION, 'identities': identities, 'cases': records, 'passed': len(records) - failed,
              'failed': failed, 'unexecuted': total - len(records),
              'source_sha256': conformance.sha256_file(Path(__file__)),
              'limitations': ['trusted bounded inputs; not a security sandbox', 'ASCII patterns; no archives or escapes']}
    with tempfile.NamedTemporaryFile(mode='w', dir=artifacts, prefix='campaign-', suffix='.json', delete=False) as out:
        json.dump(report, out, indent=2, sort_keys=True)
        out.write('\n')
        print(f'Wildcard report: {out.name}')
    print(f'Wildcard generated: passed={len(records)-failed}, failed={failed}, unexecuted={total-len(records)}')
    return int(bool(failed))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--self-test', action='store_true')
    mode.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle', type=Path, default=conformance.ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--oracle-profile', type=Path, default=oracle.PROFILE)
    parser.add_argument('--seed', type=int, default=441031)
    parser.add_argument('--cases', type=int, default=100)
    parser.add_argument('--expressions', type=int, default=32)
    parser.add_argument('--start-index', type=int, default=0)
    parser.add_argument('--artifacts', type=Path, default=conformance.ROOT / 'artifacts/generated-wildcard')
    args = parser.parse_args(argv)
    if not (0 <= args.seed < 2**63 and 0 <= args.start_index < 1000000
            and 1 <= args.cases <= 5000 and 1 <= args.expressions <= 128):
        parser.error('seed/index/cases/expressions exceed bounded campaign limits')
    try:
        reference = conformance.executable_identity(args.oracle, oracle=True)
        reference['profile'] = oracle.verify(Path(reference['path']), args.oracle_profile)
        candidate = reference if args.self_test else conformance.executable_identity(args.candidate)
        return run((generate(args.seed, index, args.expressions)
                    for index in range(args.start_index, args.start_index + args.cases)),
                   Path(reference['path']), Path(candidate['path']), args.artifacts.resolve(),
                   {'oracle': reference, 'candidate': candidate}, args.cases)
    except (conformance.HarnessError, oracle.OracleError, OSError) as error:
        print(f'wildcard-generated: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
