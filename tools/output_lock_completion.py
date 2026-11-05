#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Check output-mutex completion in five catalog profiles against two GNU runs."""

import argparse
from dataclasses import dataclass, field
import fcntl
import gettext
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import nls_diagnostics as nls
from trace_job_languages import CATALOGS
from trace_nls import CATALOG_HASH, file_entry

ROOT = Path(__file__).resolve().parents[1]
START = b'$(file >seen,anchor)\n'
FINISH = b'$(file >>seen,after)\nall:;@printf "recipe\\n"\n'
PAYLOAD = br'X:=$$(file\ >>seen,payload)'
MISSING = b'--sync-mutex=fnm:absent'
VALID = b'--sync-mutex=fnm:lock'
INVALID = b'--sync-mutex=broken'
INVALID_TAG = "invalid --sync-mutex string '%s'"
OPEN_ERROR = 'cannot open output sync mutex %s: %s'
MODE_ERROR = ("unknown output-sync type '%s'", (b'broken',))
STOP = '.  Stop.\n'


@dataclass(frozen=True)
class Model:
    name: str
    makefile: bytes
    args: tuple[bytes, ...] = (b'-Otarget',)
    inputs: dict[str, bytes] = field(default_factory=dict)
    seen: bytes | None = None
    remove: tuple[str, ...] = ()
    stdout: bytes = b''
    invalid_warnings: int = 0
    error: tuple[str, tuple[bytes, ...]] | None = None
    environment: dict[str, str] = field(default_factory=dict)
    held: str | None = None


def missing(path):
    return OPEN_ERROR, (path, b'No such file or directory')


def live(flags):
    return START + b'MAKEFLAGS += ' + flags + b'\n' + FINISH


def models():
    empty = b'all:;@:\n'
    good = {'lock': b''}
    synchronized = {'MAKEFLAGS': '-j2 -Otarget --sync-mutex=fnm:lock'}
    active_setup = (ROOT / 'tests/cases/passive-mutex-active-setup-reopens/Makefile').read_bytes()
    return (
        Model('cli-missing', empty, (b'-Otarget', MISSING), error=missing(b'absent')),
        Model('cli-inactive', empty, (b'-Onone', MISSING), error=missing(b'absent')),
        Model('cli-raw-path', empty, (b'-Otarget', b'--sync-mutex=fnm:missing-\xff'),
              error=missing(b'missing-\xff')),
        Model('cli-invalid-tag', empty, (b'-Otarget', INVALID), invalid_warnings=2),
        Model('cli-mode-first', empty, (b'--output-sync=broken', MISSING), error=MODE_ERROR),
        Model('cli-existing', b'all:;@printf "ok\\n"\n', (b'-Otarget', VALID), good, stdout=b'ok\n'),
        Model('live-later-read', b'$(info before)\nMAKEFLAGS += ' + MISSING
              + b'\n$(info after)\n' + empty, stdout=b'before\n', error=missing(b'absent')),
        Model('live-invalid-tag', live(INVALID), seen=b'anchor\nafter\n',
              stdout=b'recipe\n', invalid_warnings=2),
        Model('live-existing', live(VALID), inputs=good, seen=b'anchor\nafter\n', stdout=b'recipe\n'),
        Model('live-raw-path', live(b'--sync-mutex=fnm:missing-\xff'), seen=b'anchor\n',
              error=missing(b'missing-\xff')),
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
        Model('held-live-failed-replacement', b'$(info before)\nMAKEFLAGS += ' + MISSING
              + b'\n$(info after)\n' + empty, args=(), inputs=good,
              stdout=b'before\n', error=missing(b'absent'), environment=synchronized, held='lock'),
        Model('held-passive-failed-replacement', b'$(info before)\noverride GNUMAKEFLAGS = '
              + MISSING + b'\n$(info after)\n' + empty, args=(), inputs=good,
              stdout=b'before\nafter\n', error=missing(b'absent'), environment=synchronized,
              held='lock'),
        Model('held-active-setup-failed-replacement', active_setup, args=(b'-j2',), inputs=good,
              seen=b'reader\nreader-end\ngnu-source\ngnu-payload\nmake-source\n',
              remove=('lock',), stdout=b'gnu-source\nmake-source\n', error=missing(b'lock'),
              environment={'MAKEFLAGS': '-Otarget'}, held='lock'),
    )


def run(case, runtime, executable, held):
    if held is None:
        return h.execute(case, runtime, executable)
    h.reset_workspace(case, runtime, executable)
    # This process holds exactly the fixture lock for the bounded invocation.
    # If the child waits for a retired provider, the harness kills its group.
    with (runtime / 'work' / held).open('r+b') as lock:
        fcntl.lockf(lock, fcntl.LOCK_EX | fcntl.LOCK_NB, 1)
        out, err, status, limit, leftovers = h.capture(
            [str(runtime / 'bin/make'), *case.args], runtime / 'work',
            h.clean_environment(runtime, case.environment), case.timeout,
        )
    filesystem = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    return h.Observation(out, err, status, limit, filesystem, leftovers)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', required=True, type=Path)
    parser.add_argument('--candidate', required=True, type=Path)
    parser.add_argument('--locale-root', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    oracle, profile, french, locale = nls.prepare_reference(args.oracle, CATALOG_HASH, args.locale_root)
    engines = (oracle, oracle, args.candidate.resolve())
    report = {'oracle_profile_id': profile['profile_id'], 'locale_profile': locale,
              'binary_identities': [h.executable_identity(path) for path in engines], 'cases': []}
    catalog_root = french.parents[2]
    languages = [('C', {}, str.encode)]
    localized = {**locale['environment'], 'LC_ALL': locale['locale'], 'LANG': locale['locale']}
    for language, digest in {'fr': CATALOG_HASH, **CATALOGS}.items():
        path = Path(language) / 'LC_MESSAGES/make.mo'
        for base in (catalog_root, ROOT / 'catalogs'):
            if h.sha256_file(base / path) != digest:
                raise h.HarnessError('incorrect output lock catalog: ' + language)
        with (catalog_root / path).open('rb') as stream:
            catalog = gettext.GNUTranslations(stream)
        languages.append((language, {**localized, 'LANGUAGE': language},
                          lambda key, catalog=catalog: catalog.gettext(key).encode()))
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
            for language, environment, lookup in languages:
                stderr = b''
                if model.invalid_warnings:
                    warning = b'make: ' + (lookup(INVALID_TAG) % (b'broken',)) + b'\n'
                    stderr += warning * model.invalid_warnings
                if model.error:
                    key, arguments = model.error
                    stderr += b'make: *** ' + (lookup(key) % arguments) + lookup(STOP)
                argv = ('-rR', '--no-print-directory', *map(os.fsdecode, model.args))
                expected = {'exit_code': 2 if model.error else 0,
                            'stdout_hex': model.stdout.hex(), 'stderr_hex': stderr.hex()}
                case = h.Case(fixture, model.name + '-' + language, 'output.lock-completion',
                              'required', argv, 5, {**environment, **model.environment}, expected)
                h.reset_workspace(case, runtime, oracle)
                fs = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
                for name in model.remove:
                    del fs['work/' + name]
                if model.seen is not None:
                    fs['work/seen'] = file_entry(model.seen)
                row = {'id': case.id, 'args_hex': [os.fsencode(arg).hex() for arg in argv],
                       'expected': expected, 'expected_filesystem': fs, 'held_lock': model.held, 'runs': []}
                report['cases'].append(row)
                for engine in engines:
                    observed = run(case, runtime, engine, model.held)
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
