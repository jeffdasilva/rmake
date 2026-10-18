#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original bounded word/wordlist numeric-index conformance matrix.

Inputs have hand-authored numeric or error classifications, not a second parser.
All bytes, diagnostic locations, status and filesystem effects are anchored.
Trusted fixtures use conformance's same-alias process containment, not a security
sandbox. No GNU Make source or tests are copied into this campaign.
"""
from __future__ import annotations

import argparse
import base64
import dataclasses
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile

import conformance
import oracle

VERSION = 1
ARGS = ('-rR', '--no-print-directory')
PROTECTED = b'Original word-index campaign anchor.\n'
LIST = b'alpha  beta\tgamma'
PREFIX = b'$(file >prefix.marker,before)\n'
TAIL = b'$(file >suffix.marker,after)\nall:;@:\n'
LOW = -9223372036854775808
HIGH = 9223372036854775807


@dataclasses.dataclass(frozen=True)
class Index:
    id: str
    raw: bytes
    number: int | None = None
    kind: str = 'valid'


def indices() -> tuple[Index, ...]:
    rows = [
        Index('empty', b'', kind='empty'),
        Index('mixed-whitespace', b' \t\r\n\v\f ', kind='empty'),
        Index('zero', b'0', 0), Index('plus-zero', b'+0', 0),
        Index('minus-zero', b'-0', 0), Index('padded-minus-zero', b' -000 ', 0),
        Index('one', b'1', 1), Index('plus-one', b'+01', 1),
        Index('padded-two', b'+0002', 2), Index('three', b'3', 3),
        Index('past-list', b'4', 4), Index('decimal-leading-zero', b'010', 10),
        Index('negative', b'-1', -1), Index('padded-negative', b' -0001 ', -1),
        Index('i32-max', b'2147483647', 2147483647),
        Index('i32-over', b'2147483648', 2147483648),
        Index('u32-max', b'4294967295', 4294967295),
        Index('u32-over', b'4294967296', 4294967296),
        Index('i64-max', b'9223372036854775807', HIGH),
        Index('i64-plus-max', b'+9223372036854775807', HIGH),
        Index('i64-min', b'-9223372036854775808', LOW),
        Index('i64-over', b'9223372036854775808', kind='range'),
        Index('i64-under', b'-9223372036854775809', kind='range'),
        Index('u64-max', b'18446744073709551615', kind='range'),
        Index('u64-over', b'18446744073709551616', kind='range'),
        Index('long-zero-prefix', b'0' * 80 + b'2', 2),
        Index('long-negative-zero-prefix', b'-' + b'0' * 80 + b'1', -1),
        Index('long-overflow', b'9' * 80, kind='range'),
        Index('overflow-before-suffix', b' \t9223372036854775808tail\v ', kind='range'),
        Index('underflow-before-suffix', b'-9223372036854775809tail', kind='range'),
        Index('valid-prefix-invalid-suffix', b'9223372036854775807tail', kind='syntax'),
        Index('invalid-before-overflow', b'tail' + b'9' * 80, kind='syntax'),
        Index('plus-only', b'+', kind='syntax'), Index('minus-only', b'-', kind='syntax'),
        Index('double-sign', b'+-1', kind='syntax'), Index('repeated-sign', b'--1', kind='syntax'),
        Index('hex', b'0x2', kind='syntax'), Index('fraction', b'2.0', kind='syntax'),
        Index('exponent', b'2e0', kind='syntax'), Index('interior-space', b'1 2', kind='syntax'),
        Index('sign-space', b'+ 2', kind='syntax'), Index('quote', b"2'3", kind='syntax'),
        Index('raw-byte', b'\xff2\xfe', kind='syntax'),
        Index('control-1c', b'\x1c2\x1c', kind='syntax'),
        Index('nonascii-85', b'\x852\x85', kind='syntax'),
        Index('nonascii-a0', b'\xa02\xa0', kind='syntax'),
    ]
    for separator in (b' ', b'\t', b'\n', b'\r', b'\v', b'\f'):
        rows += [Index('surrounded-' + separator.hex(), separator + b'+0002' + separator, 2),
                 Index('empty-' + separator.hex(), separator, kind='empty')]
    return tuple(rows)


def invalid(function: bytes, ordinal: bytes, raw: bytes | None = None, *, out_of_range=False) -> bytes:
    prefix = b'invalid ' + ordinal + b" argument to '" + function + b"' function: "
    if raw is None:
        return prefix + b'empty value'
    return prefix + b"'" + raw + b"'" + (b' out of range' if out_of_range else b'')


def outcome(index: Index, role: str) -> tuple[bytes, bytes | None]:
    function = b'word' if role == 'word' else b'wordlist'
    ordinal = b'second' if role == 'stop' else b'first'
    if index.kind != 'valid':
        return b'', invalid(function, ordinal, None if index.kind == 'empty' else index.raw,
                            out_of_range=index.kind == 'range')
    number = index.number
    assert number is not None
    if role == 'word':
        if number <= 0:
            return b'', b"first argument to 'word' function must be greater than 0"
        return (b'alpha', b'beta', b'gamma')[number - 1] if number <= 3 else b'', None
    if (role == 'start' and number <= 0) or (role == 'stop' and number < 0):
        return b'', invalid(function, ordinal, str(number).encode())
    if role == 'start':
        return {1: b'alpha  beta', 2: b'beta'}.get(number, b''), None
    return b'' if number == 0 else b'alpha' if number == 1 else b'alpha  beta' if number == 2 else LIST, None


def function(name: bytes, *args: bytes) -> bytes:
    return b'$(' + name + b' ' + b','.join(args) + b')'


def diagnostic(message: bytes, line: int | None) -> bytes:
    prefix = b'make' if line is None else b'Makefile:' + str(line).encode()
    return prefix + b': *** ' + message + b'.  Stop.\n'


@dataclasses.dataclass(frozen=True)
class Cell:
    id: str
    family: str
    makefile: bytes
    stdout: bytes
    stderr: bytes
    status: int
    created: tuple[tuple[str, bytes], ...]
    args: tuple[str, ...] = ARGS
    environment: tuple[tuple[str, str], ...] = ()
    index: bytes | None = None

    def metadata(self) -> dict:
        return {'case': self.id, 'family': self.family,
                'makefile_sha256': hashlib.sha256(self.makefile).hexdigest(),
                'index_hex': None if self.index is None else self.index.hex(),
                'expected_stdout_hex': self.stdout.hex(), 'expected_stderr_hex': self.stderr.hex(),
                'expected_status': self.status, 'argv_after_alias': self.args,
                'environment': dict(self.environment),
                'expected_created_hex': {name: data.hex() for name, data in self.created}}


def direct(case_id: str, family: str, expression: bytes, value=b'', error=None,
           emitted=b'', created=(), index: bytes | None = None) -> Cell:
    prelude = (b'define RAW_INDEX\n@' + index + b'@\nendef\n'
               b'INDEX = $(subst @,,$(value RAW_INDEX))\n') if index is not None else b''
    before = prelude + PREFIX
    source = before + b'$(info [' + expression + b'])\n' + TAIL
    files = (('prefix.marker', b'before\n'), *created)
    if error is None:
        files += (('suffix.marker', b'after\n'),)
    return Cell(case_id, family, source, emitted + (b'[' + value + b']\n' if error is None else b''),
                b'' if error is None else diagnostic(error, before.count(b'\n') + 1),
                0 if error is None else 2, files, index=index)


def effect(name: str, tail=b'') -> bytes:
    word = name.encode()
    return b'$(file >' + word + b'.marker,' + word + b')$(info ' + word + b')' + tail


def marks(*names: str) -> tuple[tuple[str, bytes], ...]:
    return tuple((name + '.marker', name.encode() + b'\n') for name in names)


def inventory() -> tuple[Cell, ...]:
    cells = []
    expressions = {'word': function(b'word', b'$(INDEX)', LIST),
                   'start': function(b'wordlist', b'$(INDEX)', b'2', LIST),
                   'stop': function(b'wordlist', b'1', b'$(INDEX)', LIST)}
    for index in indices():
        for role, expression in expressions.items():
            value, error = outcome(index, role)
            cells.append(direct(role + '-' + index.id, 'index-' + role, expression,
                                value, error, index=index.raw))

    pairs = (
        ('zero-before-invalid', b'0', b'bad', b'', invalid(b'wordlist', b'first', b'0')),
        ('minus-zero-before-invalid', b'-0', b'bad', b'', invalid(b'wordlist', b'first', b'0')),
        ('negative-before-invalid', b'-001', b'bad', b'', invalid(b'wordlist', b'first', b'-1')),
        ('syntax-before-negative', b'bad', b'-1', b'', invalid(b'wordlist', b'first', b'bad')),
        ('range-before-syntax', b'9223372036854775808x', b'bad', b'',
         invalid(b'wordlist', b'first', b'9223372036854775808x', out_of_range=True)),
        ('valid-before-empty', b'1', b'', b'', invalid(b'wordlist', b'second')),
        ('negative-stop-canonical', b'1', b'-0001', b'', invalid(b'wordlist', b'second', b'-1')),
        ('zero-stop', b'2', b'0', b'', None),
        ('reversed', b'2', b'1', b'', None),
        ('same-endpoints', b'3', b'3', b'gamma', None),
        ('maximum-endpoints', b'9223372036854775807', b'9223372036854775807', b'', None),
        ('maximum-stop', b'1', b'9223372036854775807', LIST, None),
        ('maximum-stop-remainder', b'2', b'9223372036854775807', b'beta\tgamma', None),
        ('negative-zero-stop', b'1', b'-0', b'', None),
    )
    for name, first, second, value, error in pairs:
        cells.append(direct('pair-' + name, 'validation-order',
                            function(b'wordlist', first, second, LIST), value, error))
    long_list = b'w1 w2 w3 w4 w5 w6 w7 w8 w9 w10 w11 w12'
    cells.append(direct('decimal-word-leading-zero', 'decimal-grammar',
                        function(b'word', b'010', long_list), b'w10'))
    cells.append(direct('decimal-wordlist-leading-zero', 'decimal-grammar',
                        function(b'wordlist', b'08', b'010', long_list), b'w8 w9 w10'))

    def eager(name, expression, value=b'', error=None, events=(), created=None):
        cells.append(direct('effect-' + name, 'eager-effects', expression, value, error,
                            b''.join(event.encode() + b'\n' for event in events),
                            marks(*events) if created is None else created))

    bad_word = invalid(b'word', b'first', b'bad')
    bad_first = invalid(b'wordlist', b'first', b'bad')
    bad_second = invalid(b'wordlist', b'second', b'bad')
    eager('word-list-before-validation', function(b'word', b'bad', effect('list', LIST)),
          error=bad_word, events=('list',))
    eager('word-zero-list', function(b'word', b'0', effect('list', LIST)),
          error=b"first argument to 'word' function must be greater than 0", events=('list',))
    eager('word-list-error-wins', function(b'word', b'bad', effect('list', b'$(error nested)')),
          error=b'nested', events=('list',))
    eager('wordlist-all-before-validation', function(b'wordlist', effect('first', b'bad'),
                                                     effect('second', b'bad'), effect('list', LIST)),
          error=bad_first, events=('first', 'second', 'list'))
    eager('wordlist-zero-before-invalid', function(b'wordlist', effect('first', b'0'),
                                                  effect('second', b'bad'), effect('list', LIST)),
          error=invalid(b'wordlist', b'first', b'0'), events=('first', 'second', 'list'))
    eager('wordlist-second-invalid', function(b'wordlist', effect('first', b'1'),
                                              effect('second', b'bad'), effect('list', LIST)),
          error=bad_second, events=('first', 'second', 'list'))
    eager('wordlist-second-nested', function(b'wordlist', effect('first', b'bad'),
                                             effect('second', b'$(error nested)'), effect('list', LIST)),
          error=b'nested', events=('first', 'second'))
    eager('wordlist-list-nested', function(b'wordlist', effect('first', b'bad'),
                                           effect('second', b'bad'), effect('list', b'$(error nested)')),
          error=b'nested', events=('first', 'second', 'list'))

    def arity(name: bytes, count: int) -> bytes:
        return b'insufficient number of arguments (' + str(count).encode() + b") to function '" + name + b"'"

    for name in (b'word', b'wordlist'):
        label = name.decode()
        eager(label + '-provided-before-arity', function(name, effect('first', b'2')),
              error=arity(name, 1), events=('first',))
        eager(label + '-nested-before-arity', function(name, effect('first', b'$(error nested)')),
              error=b'nested', events=('first',))
        eager('call-' + label + '-zero-arity', function(b'call', name), error=arity(name, 0))
    eager('wordlist-two-before-arity', function(b'wordlist', effect('first', b'2'), effect('second', b'bad')),
          error=arity(b'wordlist', 2), events=('first', 'second'))
    eager('call-word-one-before-arity', function(b'call', b'word', effect('first', b'2')),
          error=arity(b'word', 1), events=('first',))
    eager('call-wordlist-two-before-arity', function(b'call', b'wordlist', effect('first', b'2'), effect('second', b'bad')),
          error=arity(b'wordlist', 2), events=('first', 'second'))
    eager('call-word-surplus', function(b'call', b'word', b'2', LIST, effect('extra')),
          value=b'beta', events=('extra',))
    eager('call-word-surplus-error', function(b'call', b'word', b'bad', LIST, effect('extra', b'$(error extra)')),
          error=b'extra', events=('extra',))
    eager('call-wordlist-surplus', function(b'call', b'wordlist', b'1', b'2', LIST, effect('extra')),
          value=b'alpha  beta', events=('extra',))
    eager('call-wordlist-surplus-error', function(b'call', b'wordlist', b'bad', b'bad', LIST,
                                               effect('extra', b'$(error extra)')),
          error=b'extra', events=('extra',))

    prefix_files = (('prefix.marker', b'before\n'),)
    contexts = (
        ('recursive-definition', PREFIX + b'BROKEN = $(word bad,alpha beta)\n# gap\nVALUE := $(BROKEN)\nall:;@:\n',
         bad_word, 2, ARGS, (), prefix_files, b''),
        ('nested-definition', PREFIX + b'INNER = $(wordlist 1,bad,alpha beta)\nOUTER = $(INNER)\nVALUE := $(OUTER)\nall:;@:\n',
         bad_second, 2, ARGS, (), prefix_files, b''),
        ('recipe-definition', PREFIX + b'BROKEN = $(word bad,alpha beta)\nall:\n\t@printf should-not-run\n\t@echo $(BROKEN)\n',
         bad_word, 2, ARGS, (), prefix_files, b''),
        ('recipe-direct', PREFIX + b'all:\n\t@$(file >expanded.marker,before)$(info expanded)printf should-not-run\n\t@echo $(wordlist 1,bad,alpha beta)\n',
         bad_second, 4, ARGS, (), prefix_files + (('expanded.marker', b'before\n'),), b'expanded\n'),
        ('command-line-recursive', PREFIX + b'all:;@echo $(BROKEN)\n', bad_word, 2,
         (*ARGS, 'BROKEN=$(word bad,alpha beta)'), (), prefix_files, b''),
        ('environment-recursive', PREFIX + b'all:;@echo $(BROKEN)\n', bad_word, 2,
         ARGS, (('BROKEN', '$(word bad,alpha beta)'),), prefix_files, b''),
        ('target-definition', PREFIX + b'all: BROKEN = $(word bad,alpha beta)\nall:;@echo $(BROKEN)\n',
         bad_word, 2, ARGS, (), prefix_files, b''),
        ('call-body', PREFIX + b'FAIL = $(wordlist 1,$(1),alpha beta)\nVALUE := $(call FAIL,bad)\nall:;@:\n',
         bad_second, 2, ARGS, (), prefix_files, b''),
        ('command-line-immediate', PREFIX + b'all:;@:\n', bad_word, None,
         (*ARGS, 'BROKEN:=$(word bad,alpha beta)'), (), (), b''),
    )
    for name, source, error, line, args, environment, files, emitted in contexts:
        cells.append(Cell('context-' + name, 'source-context', source, emitted,
                          diagnostic(error, line), 2, files, args, environment))
    return tuple(cells)


def regular_file(content: bytes) -> dict:
    return {'type': 'file', 'mode': 0o644, 'size': len(content),
            'content': base64.b64encode(content).decode('ascii'),
            'sha256': hashlib.sha256(content).hexdigest()}


def expected(cell: Cell) -> conformance.Observation:
    files = {'Makefile': cell.makefile, 'protected.txt': PROTECTED, **dict(cell.created)}
    filesystem = {name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp', 'work')}
    filesystem.update({'work/' + name: regular_file(content) for name, content in files.items()})
    return conformance.Observation(cell.stdout, cell.stderr, cell.status, None, filesystem)


def materialize(cell: Cell, parent: Path) -> conformance.Case:
    path = parent / cell.id
    path.mkdir(parents=True)
    for name, content in {'Makefile': cell.makefile, 'protected.txt': PROTECTED}.items():
        (path / name).write_bytes(content)
        (path / name).chmod(0o644)
    return conformance.Case(path, cell.id, 'functions.word-index', 'required', cell.args, 5,
                            dict(cell.environment), {})


def run_campaign(cells: tuple[Cell, ...], reference: Path, candidate: Path,
                 artifacts: Path, identities: dict, replay: list[str], max_failures: int = 20) -> int:
    known = {cell.id: cell for cell in inventory()}
    if (not cells or len({cell.id for cell in cells}) != len(cells)
            or any(known.get(cell.id) != cell for cell in cells)):
        raise conformance.HarnessError('select distinct original word-index cases')
    if type(max_failures) is not int or not 1 <= max_failures <= len(known):
        raise conformance.HarnessError('max-failures must be between 1 and the inventory size')
    failures = 0
    records = []
    with tempfile.TemporaryDirectory(prefix='rmake-word-index-') as temporary:
        root = Path(temporary)
        workspace = root / 'execution'
        workspace.mkdir()
        for cell in cells:
            case = materialize(cell, root / 'inputs')
            anchor = expected(cell)
            observed = {label: conformance.execute(case, workspace, binary)
                        for label, binary in (('oracle', reference), ('candidate', candidate))}
            errors = []
            for label, result in observed.items():
                errors.extend(label + ': authored ' + difference
                              for difference in conformance.differences(anchor, result))
            errors.extend(conformance.differences(observed['oracle'], observed['candidate']))
            record = {**cell.metadata(), 'status': 'FAIL' if errors else 'PASS'}
            if errors:
                failures += 1
                artifact = conformance.save_artifact(artifacts, case, workspace, identities,
                                                       observed['oracle'], observed['candidate'], errors, 'FAIL')
                shutil.copyfile(Path(__file__), artifact / 'word_index_matrix.py')
                record.update(errors=errors, artifact=str(artifact), replay_argv=replay + ['--case', cell.id])
                (artifact / 'matrix.json').write_text(json.dumps(record, indent=2, sort_keys=True) + '\n')
                print(f'FAIL {cell.id}: {", ".join(errors)}\n  evidence: {artifact}', flush=True)
            records.append(record)
            if len(records) % 32 == 0 or len(records) == len(cells):
                print(f'Progress {len(records)}/{len(cells)}; failures={failures}', flush=True)
            if failures >= max_failures:
                break
    report = {'matrix_version': VERSION, 'result': 'FAIL' if failures else 'PASS',
              'passed': len(records) - failures, 'failed': failures, 'executed': len(records),
              'unexecuted': len(cells) - len(records), 'inventory_cells': len(inventory()),
              'selected_cases': [cell.id for cell in cells], 'max_failures': max_failures,
              'identities': identities, 'cases': records, 'replay_argv': replay,
              'source_sha256': conformance.sha256_file(Path(__file__)),
              'workload_sha256': hashlib.sha256(b''.join(cell.makefile for cell in cells)).hexdigest(),
              'timeout_seconds': 5, 'limitations': ['trusted fixtures; containment is not a security sandbox',
                                                   'fixed signed-64 Linux profile; timestamps not compared']}
    reports = artifacts / 'campaigns'
    reports.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', prefix='word-index-',
                                     dir=reports, delete=False) as output:
        json.dump(report, output, indent=2, sort_keys=True)
        output.write('\n')
        print(f'Word-index campaign report: {output.name}')
    print(f'Word-index matrix: passed={len(records) - failures}, failed={failures}, '
          f'unexecuted={len(cells) - len(records)}')
    return int(bool(failures))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--self-test', action='store_true')
    mode.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle', type=Path, default=conformance.ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--oracle-profile', type=Path, default=oracle.PROFILE)
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--max-failures', type=int, default=20)
    parser.add_argument('--artifacts', type=Path, default=conformance.ROOT / 'artifacts/word-index-matrix')
    args = parser.parse_args(argv)
    try:
        ids = {cell.id for cell in inventory()}
        if set(args.case) - ids:
            raise conformance.HarnessError(f'unknown word-index case IDs: {sorted(set(args.case) - ids)}')
        if not 1 <= args.max_failures <= len(ids):
            raise conformance.HarnessError('max-failures must be between 1 and the inventory size')
        cells = tuple(cell for cell in inventory() if not args.case or cell.id in args.case)
        profile = oracle.verify(args.oracle.expanduser().resolve(), args.oracle_profile)
        reference = conformance.executable_identity(args.oracle, oracle=True)
        reference['profile'] = profile
        candidate = reference if args.self_test else conformance.executable_identity(args.candidate)
        replay = [sys.executable, str(Path(__file__).resolve()), '--oracle', reference['path'],
                  '--oracle-profile', str(args.oracle_profile.expanduser().resolve()),
                  *(['--self-test'] if args.self_test else ['--candidate', candidate['path']])]
        return run_campaign(cells, Path(reference['path']), Path(candidate['path']),
                            args.artifacts.expanduser().resolve(),
                            {'oracle': reference, 'candidate': candidate}, replay, args.max_failures)
    except (conformance.HarnessError, oracle.OracleError, OSError) as error:
        print(f'word-index-matrix: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
