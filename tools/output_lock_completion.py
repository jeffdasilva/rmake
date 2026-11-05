#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Check live output-mutex completion in C and French against two GNU runs."""

import argparse
from dataclasses import dataclass, field
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import nls_diagnostics as nls
from trace_nls import CATALOG_HASH, file_entry

ROOT = Path(__file__).resolve().parents[1]
START = b'$(file >seen,anchor)\n'
FINISH = b'$(file >>seen,after)\nall:;@printf "recipe\\n"\n'
PAYLOAD = br'X:=$$(file\ >>seen,payload)'
MISSING = b'--sync-mutex=fnm:absent'
VALID = b'--sync-mutex=fnm:lock'
INVALID = b'--sync-mutex=broken'
WARNING = b"make: invalid --sync-mutex string 'broken'\n"
MODE_ERROR = (b"unknown output-sync type 'broken'",
              'type de output-sync «\xa0broken\xa0» inconnu'.encode())


@dataclass(frozen=True)
class Model:
    name: str
    makefile: bytes
    args: tuple[bytes, ...] = (b'-Otarget',)
    inputs: dict[str, bytes] = field(default_factory=dict)
    seen: bytes | None = None
    remove: tuple[str, ...] = ()
    stdout: bytes = b''
    warning: bytes = b''
    error: tuple[bytes, bytes] | None = None


def missing(path):
    message = b'cannot open output sync mutex ' + path + b': No such file or directory'
    return message, message


def live(flags):
    return START + b'MAKEFLAGS += ' + flags + b'\n' + FINISH


def models():
    empty = b'all:;@:\n'
    good = {'lock': b''}
    return (
        Model('cli-missing', empty, (b'-Otarget', MISSING), error=missing(b'absent')),
        Model('cli-inactive', empty, (b'-Onone', MISSING), error=missing(b'absent')),
        Model('cli-raw-path', empty, (b'-Otarget', b'--sync-mutex=fnm:missing-\xff'),
              error=missing(b'missing-\xff')),
        Model('cli-invalid-tag', empty, (b'-Otarget', INVALID), warning=WARNING * 2),
        Model('cli-mode-first', empty, (b'--output-sync=broken', MISSING), error=MODE_ERROR),
        Model('cli-existing', b'all:;@printf "ok\\n"\n', (b'-Otarget', VALID), good, stdout=b'ok\n'),
        Model('live-later-read', b'$(info before)\nMAKEFLAGS += ' + MISSING
              + b'\n$(info after)\n' + empty, stdout=b'before\n', error=missing(b'absent')),
        Model('live-invalid-tag', live(INVALID), seen=b'anchor\nafter\n',
              stdout=b'recipe\n', warning=WARNING * 2),
        Model('live-existing', live(VALID), inputs=good, seen=b'anchor\nafter\n', stdout=b'recipe\n'),
        Model('live-last-existing', live(MISSING + b' ' + VALID), inputs=good,
              seen=b'anchor\nafter\n', stdout=b'recipe\n'),
        Model('live-last-missing', live(VALID + b' ' + MISSING), inputs=good,
              seen=b'anchor\n', error=missing(b'absent')),
        Model('live-payload-before', live(PAYLOAD + b' ' + MISSING), seen=b'anchor\npayload\n',
              error=missing(b'absent')),
        Model('live-payload-after', live(MISSING + b' ' + PAYLOAD), seen=b'anchor\npayload\n',
              error=missing(b'absent')),
        Model('live-mode-first', live(b'--output-sync=broken ' + MISSING + b' ' + PAYLOAD),
              seen=b'anchor\npayload\n', error=MODE_ERROR),
        Model('live-include', START + b'include reader.mk\n' + FINISH,
              inputs={'reader.mk': b'$(file >>seen,include-before)\nMAKEFLAGS += ' + MISSING
                      + b'\n$(file >>seen,include-after)\n'}, seen=b'anchor\ninclude-before\n',
              error=missing(b'absent')),
        Model('live-reopen', START + b'MAKEFLAGS += ' + VALID
              + b'\n$(shell rm lock)\nMAKEFLAGS += --silent\n' + FINISH,
              inputs=good, seen=b'anchor\n', remove=('lock',), error=missing(b'lock')),
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', required=True, type=Path)
    parser.add_argument('--candidate', required=True, type=Path)
    parser.add_argument('--locale-root', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    oracle, profile, _, locale = nls.prepare_reference(args.oracle, CATALOG_HASH, args.locale_root)
    engines = (oracle, oracle, args.candidate.resolve())
    report = {'oracle_profile_id': profile['profile_id'], 'locale_profile': locale,
              'binary_identities': [h.executable_identity(path) for path in engines], 'cases': []}
    languages = (('C', {}, b'.  Stop.\n'),
                 ('fr', {**locale['environment'], 'LC_ALL': locale['locale'], 'LANG': locale['locale']},
                  '. Arrêt.\n'.encode()))
    inventory = models()
    with tempfile.TemporaryDirectory(prefix='rmake-output-lock-') as directory:
        root = Path(directory)
        runtime = root / 'runtime'
        runtime.mkdir()
        for model in inventory:
            fixture = root / model.name
            fixture.mkdir()
            for name, contents in {'Makefile': model.makefile, **model.inputs}.items():
                (fixture / name).write_bytes(contents)
            for index, (language, environment, stop) in enumerate(languages):
                stderr = model.warning
                if model.error:
                    stderr += b'make: *** ' + model.error[index] + stop
                argv = ('-rR', '--no-print-directory', *map(os.fsdecode, model.args))
                expected = {'exit_code': 2 if model.error else 0,
                            'stdout_hex': model.stdout.hex(), 'stderr_hex': stderr.hex()}
                case = h.Case(fixture, model.name + '-' + language, 'output.lock-completion',
                              'required', argv, 5, environment, expected)
                h.reset_workspace(case, runtime, oracle)
                fs = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
                for name in model.remove:
                    del fs['work/' + name]
                if model.seen is not None:
                    fs['work/seen'] = file_entry(model.seen)
                row = {'id': case.id, 'args_hex': [os.fsencode(arg).hex() for arg in argv],
                       'expected': expected, 'expected_filesystem': fs, 'runs': []}
                report['cases'].append(row)
                for engine in engines:
                    observed = h.execute(case, runtime, engine)
                    errors = h.expectation_errors(case, observed)
                    if observed.filesystem != fs:
                        errors.append('complete filesystem')
                    row['runs'].append({**h.observation_json(observed), 'stdout_hex': observed.stdout.hex(),
                                        'stderr_hex': observed.stderr.hex(), 'differences': errors})
                    if observed.returncode < 0 or observed.limit or observed.leftover_processes:
                        break
                row['passed'] = len(row['runs']) == len(engines) and not any(
                    run['differences'] for run in row['runs'])
                print('PASS' if row['passed'] else 'FAIL', case.id,
                      [run['differences'] for run in row['runs']], flush=True)
    passed = sum(row['passed'] for row in report['cases'])
    report['totals'] = {'cases': len(inventory) * len(languages), 'pass': passed}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return int(passed != report['totals']['cases'])


if __name__ == '__main__':
    raise SystemExit(main())
