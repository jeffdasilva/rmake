#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original byte-exact function whitespace matrix against GNU Make 4.4.1.

Fixed authored expectations cover six C-locale separators and three bytes that
must remain ordinary data. Trusted fixtures use conformance's bounded same-alias
runner, not a filesystem security sandbox. No GNU source or tests are copied.
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

VERSION = 2
SEPARATORS = (b' ', b'\t', b'\n', b'\r', b'\v', b'\f', b'\x1c', b'\x85', b'\xa0')
C_WHITESPACE = b' \t\n\r\v\f'
PROTECTED = b'Original byte-whitespace campaign anchor.\n'
ARGS = ('-rR', '--no-print-directory')
LIMITATIONS = [
    'trusted fixed inputs and process containment, not a security sandbox',
    'single invocations; timestamps and arbitrary process events not compared',
    'numeric word/wordlist arguments have a separate matrix and curated cases',
    'GNU Make 4.4.1 crashes for multi-name let with a whitespace-only list; excluded here and retained separately as pending, never counted as a pass',
]


def function(name: bytes, *args: bytes) -> bytes:
    return b'$(' + name + b' ' + b','.join(args) + b')'


@dataclasses.dataclass(frozen=True)
class Cell:
    id: str
    family: str
    separator: bytes
    expression: bytes
    value: bytes
    emitted: bytes = b''
    created: tuple[tuple[str, bytes], ...] = ()
    error: bytes | None = None

    @property
    def prelude(self) -> bytes:
        return (b'define SEP_SOURCE\nx' + self.separator + b'x\nendef\n'
                b'SEP = $(subst x,,$(value SEP_SOURCE))\nEMPTY :=\n')

    @property
    def makefile(self) -> bytes:
        return (self.prelude + b'define E\n' + self.expression + b'\nendef\n'
                b'$(file >prefix.marker,before)\n$(info [$(E)])\n'
                b'$(file >suffix.marker,after)\nall:;@:\n')

    @property
    def stdout(self) -> bytes:
        return self.emitted + (b'[' + self.value + b']\n' if self.error is None else b'')

    @property
    def stderr(self) -> bytes:
        if self.error is None:
            return b''
        line = self.makefile.split(b'$(info [$(E)])', 1)[0].count(b'\n') + 1
        return b'Makefile:' + str(line).encode() + b': *** ' + self.error + b'.  Stop.\n'

    def metadata(self) -> dict:
        return {'case': self.id, 'family': self.family, 'separator_hex': self.separator.hex(),
                'expression_hex': self.expression.hex(),
                'makefile_sha256': hashlib.sha256(self.makefile).hexdigest(),
                'expected_stdout_hex': self.stdout.hex(), 'expected_stderr_hex': self.stderr.hex(),
                'expected_status': 2 if self.error else 0,
                'expected_created_hex': {name: content.hex() for name, content in self.created}}


def inventory() -> tuple[Cell, ...]:
    cases = []
    for separator in SEPARATORS:
        ws = separator
        split = ws in C_WHITESPACE
        text = ws + b'b.c' + ws + ws + b'a.h' + ws + b'b.c' + ws
        normalized = b'b.c a.h b.c' if split else text
        last = ws + b'LAST' + ws

        def add(family, expression, value=b'', emitted=b'', created=(), error=None):
            cases.append(Cell(f'{family}-{ws.hex()}', family, ws, expression, value,
                              emitted, tuple(created), error))

        # These are literal expected outcomes for one fixed three-word payload,
        # not calls into the candidate or a reimplementation of its tokenizer.
        word_cases = (
            ('strip', (text,), normalized),
            ('sort', (text,), b'a.h b.c' if split else text),
            ('words', (text,), b'3' if split else b'1'),
            ('firstword', (text,), b'b.c' if split else text),
            ('lastword', (text,), b'b.c' if split else text),
            ('word', (b'2', text), b'a.h' if split else b''),
            ('wordlist', (b'1', b'2', text), b'b.c' + ws + ws + b'a.h' if split else text),
            ('patsubst', (b'%.c', b'%.o', text), b'b.o a.h b.o' if split else text),
            ('filter', (b'%.c', text), b'b.c b.c' if split else b''),
            ('filter-out', (b'%.c', text), b'a.h' if split else text),
            ('join', (b'A' + ws + b'B', text), b'Ab.c Ba.h b.c' if split else b'A' + ws + b'B' + text),
            ('addprefix', (b'P', text), b'Pb.c Pa.h Pb.c' if split else b'P' + text),
            ('addsuffix', (b'S', text), b'b.cS a.hS b.cS' if split else text + b'S'),
            ('dir', (text,), b'./ ./ ./' if split else b'./'),
            ('notdir', (text,), normalized),
            ('basename', (text,), b'b a b' if split else text[:-len(ws) - 2]),
            ('suffix', (text,), b'.c .h .c' if split else b'.c' + ws),
        )
        for name, args, value in word_cases:
            add('word-' + name, function(name.encode(), *args), value)

        add('if-raw', function(b'if', ws, b'TRUE', b'FALSE'), b'FALSE' if split else b'TRUE')
        add('if-expanded', function(b'if', b'$(SEP)', ws + b'TRUE' + ws, b'FALSE'), ws + b'TRUE' + ws)
        add('if-empty-reference', function(b'if', ws + b'$(EMPTY)' + ws, b'TRUE', b'FALSE'),
            b'FALSE' if split else b'TRUE')
        add('or-raw', function(b'or', ws, last), b'LAST' if split else ws)
        add('or-expanded', function(b'or', b'$(SEP)', b'$(error unselected)'), ws)
        add('and-raw', function(b'and', ws, last), b'' if split else last)
        add('and-expanded', function(b'and', b'$(SEP)', b'$(SEP)LAST$(SEP)'), last)
        add('or-empty-effects', function(b'or', b'$(file >first.marker,first)',
                                         b'$(file >second.marker,second)' + last,
                                         b'$(error unselected)'),
            ws + b'LAST' if split else last,
            created=(('first.marker', b'first\n'), ('second.marker', b'second\n')))

        values = b'a' + ws + b'b'
        names = ws + b'x' + ws + b'ignored' + ws
        add('foreach-name', function(b'foreach', names, values, b'[$(x)]'),
            b'[a] [b]' if split else b'[]')
        add('foreach-expanded-name', function(b'foreach', b'$(SEP)x$(SEP)ignored$(SEP)', values, b'[$(x)]'),
            b'[a] [b]' if split else b'[]')
        add('let-final-remainder', function(b'let', b'x' + ws + b'y',
                                           ws + b'a' + ws + b'b' + ws + ws + b'c' + ws,
                                           b'[$(x)][$(y)]'),
            b'[a][b' + ws + ws + b'c' + ws + b']' if split else b'[][]')
        add('let-one-remainder', function(b'let', b'x', ws + b'a' + ws + b'b' + ws, b'[$(x)]'),
            b'[' + (b'a' if split else ws + b'a') + ws + b'b' + ws + b']')
        add('call-or', function(b'call', b'or', ws, last), b'LAST' if split else ws)
        add('call-if', function(b'call', b'if', ws, b'TRUE', b'FALSE'), b'FALSE' if split else b'TRUE')

        selected = b'false' if split else b'true'
        add('if-branch-effects', function(b'if', ws + b'$(EMPTY)' + ws,
                                         b'$(file >chosen.txt,true)$(info branch-true)TRUE',
                                         b'$(file >chosen.txt,false)$(info branch-false)FALSE'),
            selected.upper(), b'branch-' + selected + b'\n', (('chosen.txt', selected + b'\n'),))
        add('and-fatal-effects', function(b'and', ws, b'$(file >visited.txt,yes)$(error visited)'),
            created=() if split else (('visited.txt', b'yes\n'),),
            error=None if split else b'visited')
    return tuple(cases) + span_cases()


def span_cases() -> tuple[Cell, ...]:
    """Fixed range and byte-boundary cases for all three span consumers."""
    cases = []

    def add(name, family, expression, value, emitted=b'', created=()):
        # Literal CRLF is normalized while reading Makefiles. Keep CR and LF
        # apart in source so the empty reference recreates both value bytes.
        expression = expression.replace(b'\r\n', b'\r$(EMPTY)\n')
        cases.append(Cell('span-' + name, 'span-' + family, b'\t', expression,
                          value, emitted, tuple(created)))

    maximum = b'9223372036854775807'
    mixed = b' \tfirst\v\vsecond\r\nthird\f fourth  '
    large = b'\t ' + b'\r\n'.join(f'w{index:04d}'.encode() for index in range(4096)) + b'\f'
    ranges = (
        ('prefix', b'1', b'2', mixed, b'first\v\vsecond'),
        ('middle', b'2', b'3', mixed, b'second\r\nthird'),
        ('tail', b'3', maximum, mixed, b'third\f fourth'),
        ('empty', b'1', maximum, b'', b''),
        ('whitespace-only', b'1', b'1', b' \t\n\r\v\f', b''),
        ('large-prefix', b'1', b'3', large, b'w0000\r\nw0001\r\nw0002'),
        ('large-middle', b'2048', b'2050', large, b'w2047\r\nw2048\r\nw2049'),
        ('large-tail', b'4094', maximum, large, b'w4093\r\nw4094\r\nw4095'),
        ('large-outside', maximum, maximum, large, b''),
    )
    for name, start, end, text, result in ranges:
        add('wordlist-' + name, 'wordlist', function(b'wordlist', start, end, text), result)
    add('wordlist-prefix-eager-tail', 'wordlist',
        function(b'wordlist', b'1', b'1', b'first ' + large
                 + b'$(file >tail.marker,tail)$(info tail-read)'),
        b'first', b'tail-read\n', (('tail.marker', b'tail\n'),))

    add('let-mixed-remainder', 'let',
        function(b'let', b'x y rest', mixed, b'[$(x)][$(y)][$(rest)]'),
        b'[first][second][third\f fourth  ]')
    add('let-duplicate-binding', 'let',
        function(b'let', b'x x', b'first second\tthird  ', b'[$(x)]'),
        b'[second\tthird  ]')
    add('let-restores-binding', 'let',
        function(b'let', b'x', b'outer', b'$(let x,inner,$(x))|$(x)') + b'|$(origin x)',
        b'inner|outer|undefined')
    add('let-simple-remainder', 'let',
        function(b'let', b'x', b'first second  ', b'$(flavor x):$(value x):$(x)'),
        b'simple:first second  :first second  ')
    add('let-eager-values', 'let',
        function(b'let', b'x y', b'$(file >values.marker,values)first second third$(info values)',
                 b'[$(x)][$(y)]'), b'[first][second third]', b'values\n',
        (('values.marker', b'values\n'),))
    add('let-eager-order', 'let',
        function(b'let', b'$(file >names.marker,names)$(info names)x y',
                 b'$(file >values.marker,values)$(info values)first second',
                 b'$(file >body.marker,body)$(info body)[$(x)][$(y)]'),
        b'[first][second]', b'names\nvalues\nbody\n',
        (('names.marker', b'names\n'), ('values.marker', b'values\n'), ('body.marker', b'body\n')))
    add('let-zero-names-eager', 'let',
        function(b'let', b'$(EMPTY)', b'$(file >values.marker,values)$(info values)nonempty',
                 b'$(file >body.marker,body)$(info body)done'),
        b'done', b'values\nbody\n', (('values.marker', b'values\n'), ('body.marker', b'body\n')))
    add('let-large-remainder', 'let',
        function(b'let', b'head rest', large,
                 b'$(wordlist 1,3,$(rest))|$(words $(rest))|$(lastword $(rest))'),
        b'w0001\r\nw0002\r\nw0003|4095|w4095')

    literal = b' \tcat\vcat\r\nbob\fcat  '
    add('patsubst-literal-delimiters', 'patsubst', function(b'patsubst', b'cat', b'DOG', literal),
        b' \tDOG\vDOG\r\nbob\fDOG  ')
    negative = b' \tcatfish\vcat\x1ccat\x85cat\xa0cat\r\n'
    add('patsubst-literal-controls', 'patsubst', function(b'patsubst', b'cat', b'DOG', negative), negative)
    add('patsubst-literal-delete', 'patsubst', function(b'patsubst', b'cat', b'', literal),
        b' \t\v\r\nbob\f  ')
    add('patsubst-literal-replacement-delimiters', 'patsubst',
        function(b'patsubst', b'cat', b'A\tB', b' cat\vcat '), b' A\tB\vA\tB ')
    add('patsubst-literal-percent', 'patsubst',
        function(b'patsubst', b'c\\%t', b'R', b' c%t\vcat\fc%t '), b' R\vcat\fR ')
    add('patsubst-literal-large', 'patsubst',
        function(b'let', b'result', function(b'patsubst', b'w0000', b'replacement', large),
                 b'$(firstword $(result))|$(words $(result))|$(lastword $(result))'),
        b'replacement|4096|w4095')
    return tuple(cases)


def regular_file(content: bytes) -> dict:
    return {'type': 'file', 'mode': 0o644, 'size': len(content),
            'content': base64.b64encode(content).decode('ascii'),
            'sha256': hashlib.sha256(content).hexdigest()}


def expected(cell: Cell) -> conformance.Observation:
    files = {'Makefile': cell.makefile, 'protected.txt': PROTECTED, 'prefix.marker': b'before\n',
             **dict(cell.created)}
    if cell.error is None:
        files['suffix.marker'] = b'after\n'
    filesystem = {name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp', 'work')}
    filesystem.update({'work/' + name: regular_file(content) for name, content in files.items()})
    return conformance.Observation(cell.stdout, cell.stderr, 2 if cell.error else 0, None, filesystem)


def materialize(cell: Cell, parent: Path) -> conformance.Case:
    path = parent / cell.id
    path.mkdir(parents=True)
    for name, content in {'Makefile': cell.makefile, 'protected.txt': PROTECTED}.items():
        (path / name).write_bytes(content)
        (path / name).chmod(0o644)
    return conformance.Case(path, cell.id, 'functions.byte-whitespace', 'required', ARGS, 5, {}, {})


def run_campaign(cells: tuple[Cell, ...], reference: Path, candidate: Path,
                 artifacts: Path, identities: dict, replay: list[str], max_failures: int = 20) -> int:
    known = {cell.id: cell for cell in inventory()}
    if (not cells or len({cell.id for cell in cells}) != len(cells)
            or any(known.get(cell.id) != cell for cell in cells)):
        raise conformance.HarnessError('select distinct original whitespace cases')
    if type(max_failures) is not int or not 1 <= max_failures <= len(known):
        raise conformance.HarnessError('max-failures must be between 1 and the inventory size')
    failures = 0
    records = []
    with tempfile.TemporaryDirectory(prefix='rmake-whitespace-') as temporary:
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
                shutil.copyfile(Path(__file__), artifact / 'whitespace_matrix.py')
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
              'timeout_seconds': 5, 'limitations': LIMITATIONS}
    reports = artifacts / 'campaigns'
    reports.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', prefix='whitespace-',
                                     dir=reports, delete=False) as output:
        json.dump(report, output, indent=2, sort_keys=True)
        output.write('\n')
        print(f'Whitespace campaign report: {output.name}')
    print(f'Whitespace matrix: passed={len(records) - failures}, failed={failures}, '
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
    parser.add_argument('--artifacts', type=Path, default=conformance.ROOT / 'artifacts/whitespace-matrix')
    args = parser.parse_args(argv)
    try:
        ids = {cell.id for cell in inventory()}
        if set(args.case) - ids:
            raise conformance.HarnessError(f'unknown whitespace case IDs: {sorted(set(args.case) - ids)}')
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
        print(f'whitespace-matrix: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
