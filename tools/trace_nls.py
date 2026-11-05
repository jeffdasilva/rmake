#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact C and French trace streams, rebuild reasons, and filesystem effects."""
from dataclasses import dataclass, replace
import argparse
import base64
import gettext
import hashlib
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import nls_diagnostics as nls

CATALOG_HASH = 'dd9f6c8d75cf94c0f4a01e4ad3ee2b833b4b552f13d0adf461b83a1356ed17a4'
ROOT = Path(h.__file__).resolve().parents[1]
OLD = 946684800
NEW = 978307200
OLDER = 915148800


@dataclass(frozen=True)
class Trace:
    index: int
    location: bytes | None
    target: bytes
    reason: str | bytes

    def line(self, lookup):
        # The type distinguishes a fixed message ID from raw prerequisite bytes.
        arguments = (lookup('<builtin>') if self.location is None else self.location, self.target)
        if isinstance(self.reason, str):
            key = "%s: update target '%s' due to: " + self.reason
        else:
            key = "%s: update target '%s' due to: %s"
            arguments += (self.reason,)
        return lookup(key) % arguments + b'\n'


@dataclass(frozen=True)
class Model:
    case: h.Case
    lines: tuple[Trace, ...]
    created: dict[str, bytes | None]
    mtimes: dict[str, int]


def models(root):
    def sealed(name, lines=(), created=None, mtimes=None):
        case = h.load_case(ROOT / 'tests/cases' / ('trace-' + name))
        return Model(case, tuple(lines), created or {}, mtimes or {})

    missing = 'target does not exist'
    touched = {'out': b'', 'newer': b'', 'older': b''}
    touched_times = {'out': OLD, 'newer': NEW, 'older': OLDER}
    result = [
        sealed('phony-parallel-sync', [Trace(0, b'Makefile:4', b'all', 'target is .PHONY')]),
        sealed('missing-dry', [Trace(0, b'Makefile:3', b'all', missing)]),
        sealed('missing-query', [Trace(0, b'Makefile:3', b'all', missing)]),
        sealed('existing-newer-duplicates-normal', [Trace(0, b'Makefile:4', b'out', b'newer')], touched, touched_times),
        sealed('existing-missing-order-dry', [Trace(0, b'Makefile:4', b'out', 'unknown reasons')], touched, touched_times),
        sealed('raw-normal', [Trace(0, b'Makefile:4', b'raw\xff', missing)]),
        sealed('builtin', [Trace(0, None, b'x.o', missing)]),
        sealed('config-order', [Trace(1, b'Makefile:4', b'all', missing)]),
        sealed('expansion-fatal'),
        sealed('restart', [Trace(0, b'Makefile:6', b'all', missing)], {'inc.mk': b'# generated\n'}),
        sealed('two-lines-normal', [Trace(0, b'Makefile:4', b'all', missing)]),
        sealed('recursion', [Trace(0, b'Makefile:4', b'all', missing), Trace(2, b'child.mk:1', b'all', missing)]),
        sealed('vpath-explicit-keep', [Trace(0, b'Makefile:7', b'results/x.out', b'x.in')],
               {'results': None, 'results/x.out': b'', 'x.in': b'', 'results/x.in': b''},
               {'results/x.out': OLD, 'x.in': NEW, 'results/x.in': NEW}),
    ]

    def original(name, files, line, command, effect=None, raw_stdout=b''):
        fixture = root / name
        fixture.mkdir()
        for file, (content, mtime) in files.items():
            path = fixture / os.fsdecode(file)
            path.write_bytes(content)
            if mtime is not None:
                os.utime(path, (mtime, mtime))
        stdout = line.line(str.encode) + command + b'\n' + raw_stdout
        case = h.Case(fixture, name, 'debug.trace.locale', 'required',
                      ('-rR', '--no-print-directory', '--trace'), 5, {},
                      {'exit_code': 0, 'stdout_hex': stdout.hex(), 'stderr_hex': ''})
        times = {os.fsdecode(file): mtime for file, (_, mtime) in files.items() if mtime is not None}
        return Model(case, (line,), effect or {}, times)

    raw_recipe = b'recipe-\xfe.mk'
    raw_target = b'out-\xfc'
    raw_prereq = b'newer-\xff'
    command = b"printf 'effect\\n' > seen"
    result.append(original('trace-locale-raw-generic', {
        b'Makefile': (b'include ' + raw_recipe + b'\n', None),
        raw_recipe: (raw_target + b': ' + raw_prereq + b';@' + command + b'\n', None),
        raw_target: (b'', OLD),
        raw_prereq: (b'', NEW),
    }, Trace(0, raw_recipe + b':1', raw_target, raw_prereq), command, {'seen': b'effect\n'}))

    line = Trace(0, b'Makefile:1', b'out', b'unknown reasons')
    raw_stdout = line.line(str.encode)
    command = b'printf \'%s\\n\' "' + raw_stdout.rstrip(b'\n') + b'"'
    result.append(original('trace-locale-raw-lookalike', {
        b'Makefile': (b'out: unknown\\ reasons;@' + command + b'\n', None),
        b'out': (b'', OLD),
        b'unknown reasons': (b'', NEW),
    }, line, command, raw_stdout=raw_stdout))
    return result


def localized(model, lookup, environment):
    case = model.case
    stdout = bytes.fromhex(case.expected['stdout_hex']).splitlines(keepends=True)
    for line in model.lines:
        if stdout[line.index] != line.line(str.encode):
            raise h.HarnessError('declared trace event disagrees with the original C model')
        stdout[line.index] = line.line(lookup)
    stderr = bytes.fromhex(case.expected['stderr_hex'])
    if case.id == 'trace-expansion-fatal':
        if stderr != b'Makefile:3: *** broken.  Stop.\n':
            raise h.HarnessError('unexpected original expansion-fatal fixture')
        stderr = b'Makefile:3: *** broken' + lookup('.  Stop.\n')
    return replace(case, expected={**case.expected, 'stdout_hex': b''.join(stdout).hex(),
                                   'stderr_hex': stderr.hex()},
                   environment={**case.environment, **environment})


def file_entry(content):
    if content is None:
        return {'type': 'directory', 'mode': 0o755}
    return {'type': 'file', 'mode': 0o644, 'size': len(content),
            'sha256': hashlib.sha256(content).hexdigest(),
            'content': base64.b64encode(content).decode()}


def prepare(model, case, runtime, oracle):
    h.reset_workspace(case, runtime, oracle)
    fs = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    for path, data in model.created.items():
        key = 'work/' + path
        if key in fs:
            raise h.HarnessError('expected a new, previously absent trace effect: ' + path)
        fs[key] = file_entry(data)
    return fs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--locale-root', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--declarations', type=Path)
    parser.add_argument('--case', action='append')
    args = parser.parse_args()
    oracle, profile, catalog_path, locale = nls.prepare_reference(args.oracle, CATALOG_HASH, args.locale_root)
    with catalog_path.open('rb') as file:
        catalog = gettext.GNUTranslations(file)
    languages = [('C', str.encode, {}), ('fr', lambda key: catalog.gettext(key).encode(),
                  {**locale['environment'], 'LC_ALL': locale['locale'], 'LANG': locale['locale']})]
    engines = [oracle, oracle]
    if args.candidate:
        engines.append(args.candidate.resolve())
    report = {'classification': 'Strict trace locale with independently declared full streams and effects',
              'oracle_profile_id': profile['profile_id'], 'catalog_sha256': CATALOG_HASH,
              'locale_profile': locale, 'binary_identities': [h.executable_identity(path) for path in engines],
              'source_sha256': h.sha256_file(Path(__file__)),
              'functional_cpu_affinity': sorted(os.sched_getaffinity(0)), 'cases': []}
    with tempfile.TemporaryDirectory(prefix='rmake-trace-nls-') as directory:
        root = Path(directory)
        runtime = root / 'runtime'
        runtime.mkdir()
        inventory = models(root)
        if args.case and not set(args.case) <= {item.case.id for item in inventory}:
            parser.error('unknown exact trace case')
        declarations = []
        for model in inventory:
            if args.case and model.case.id not in args.case:
                continue
            for language, lookup, environment in languages:
                case = localized(model, lookup, environment)
                fs = prepare(model, case, runtime, oracle)
                row = {'id': case.id + '-' + language, 'args': case.args,
                       'environment': case.environment, 'expected': case.expected,
                       'expected_filesystem': fs, 'expected_mtimes_ns': {name: value * 10**9 for name, value in model.mtimes.items()},
                       'fixture': h.snapshot({'work': runtime / 'work'})}
                declarations.append((case, row))
        report['declared'] = len(declarations)
        if args.declarations:
            with args.declarations.open('x') as output:
                json.dump([row for _, row in declarations], output, indent=2)
        for case, declaration in declarations:
            row = dict(declaration, observations=[])
            for executable in engines:
                value = h.execute(case, runtime, executable)
                errors = h.expectation_errors(case, value)
                if value.filesystem != row['expected_filesystem']:
                    errors.append('independent complete filesystem')
                observed_times = {}
                for name, expected in row['expected_mtimes_ns'].items():
                    path = runtime / 'work' / name
                    stamp = path.stat().st_mtime_ns if path.is_file() else None
                    observed_times[name] = stamp
                    if stamp != expected:
                        errors.append('exact fixed mtime ' + name)
                row['observations'].append({**h.observation_json(value), 'stdout_hex': value.stdout.hex(),
                                            'stderr_hex': value.stderr.hex(), 'mtime_ns': observed_times,
                                            'differences': errors})
                if value.limit or value.leftover_processes or value.returncode < 0:
                    row['zero_credit'] = True
                    break
            row['passed'] = not any(record['differences'] for record in row['observations'])
            report['cases'].append(row)
            print('PASS' if row['passed'] else 'FAIL', row['id'],
                  [run['differences'] for run in row['observations']], flush=True)
            if row.get('zero_credit'):
                break
    report['totals'] = {'cases': len(report['cases']), 'pass': sum(row['passed'] for row in report['cases'])}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return int(report['totals']['pass'] != report['declared'])


if __name__ == '__main__':
    raise SystemExit(main())
