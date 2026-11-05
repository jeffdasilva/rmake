#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Check every owned language, fallback chains and exact GNU parser diagnostics."""
import argparse
import gettext
import json
import os
from pathlib import Path
import shutil
import tempfile

import catalogs
import conformance as h
import locale_data
import oracle

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'catalogs'
FIXTURE = ROOT / 'tests/catalog-coverage'


def translator(paths, chain, c_locale):
    if c_locale:
        return gettext.NullTranslations()
    head = gettext.NullTranslations()
    for language in chain.split(':'):
        if path := paths.get(language):
            with (ASSETS / path).open('rb') as stream:
                head.add_fallback(gettext.GNUTranslations(stream))
    return head


def observe(executable, invoke, reset, probe, locale, chain, c_locale, translation):
    invoke.unlink(missing_ok=True)
    invoke.symlink_to(executable)
    reset.mkdir()
    work, home, temporary = (reset / name for name in ('work', 'home', 'tmp'))
    for directory in (work, home, temporary):
        directory.mkdir()
    shutil.copyfile(FIXTURE / 'Makefile', work / 'Makefile')
    shutil.copyfile(probe, work / 'catalog_probe.so')
    roots = {'work': work, 'home': home, 'tmp': temporary}
    before = h.snapshot(roots)
    env = oracle.clean_environment(home) | locale['environment'] | {
        'LC_ALL': 'C' if c_locale else 'fr_FR.UTF-8', 'LANGUAGE': chain,
        'TMPDIR': str(temporary),
    }
    out, err, status, limit, children = h.capture([str(invoke), '-rR', '--no-print-directory'], work, env, 8)
    expected_out = ('catalog|' + translation.gettext('No') + '|' + translation.gettext('No targets') + '\n').encode()
    body = (translation.gettext('missing separator') + translation.gettext('.  Stop.\n')).encode()
    result = {'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status, 'limit': limit,
              'leftover_processes': children, 'files': h.snapshot(roots)}
    errors = [label for label, value in (
        ('stdout', out == expected_out),
        ('located parser message', err.startswith(b'Makefile:5: ') and err.endswith(body)),
        ('status or containment', (status, limit, children) == (2, None, False)),
        ('filesystem', result['files'] == before),
    ) if not value]
    return result | {'errors': errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--locale-root', type=Path, default=ROOT / '.tools/locale-data')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case')
    args = parser.parse_args()
    catalogs.verify(ASSETS)
    paths = catalogs.catalog_paths(ASSETS)
    reference = oracle.verify(args.oracle)
    prefixes = [flag.removeprefix('--prefix=') for flag in reference['build']['configure_args'] if flag.startswith('--prefix=')]
    if len(prefixes) != 1 or not Path(prefixes[0]).is_absolute():
        raise h.HarnessError('reference must identify a configured catalog prefix')
    reference_root = Path(prefixes[0]) / 'share/locale'
    for name in paths.values():
        if h.sha256_file(reference_root / name) != h.sha256_file(ASSETS / name):
            raise h.HarnessError('reference catalog identity differs: ' + name)
    locale = locale_data.provision(args.locale_root.resolve())
    rows = [(name, name, False) for name in paths] + [
        ('unavailable-language', 'zz_ZZ:de:fr', False),
        ('partial-language-fallback', 'be:fr', False),
        ('c-disables-language', 'de:fr', True),
    ]
    if args.case:
        rows = [row for row in rows if row[0] == args.case]
        if not rows:
            parser.error(f'unknown catalog case: {args.case}')
    results = []
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-catalog-coverage-') as directory:
            root = Path(directory)
            probe = root / 'catalog_probe.so'
            out, err, status, limit, children = h.capture(
                ['/usr/bin/cc', '-shared', '-fPIC', '-Werror', '-Wall', '-Wextra', '-o',
                 str(probe), str(FIXTURE / 'catalog_probe.c')], root, oracle.clean_environment(), 30)
            if (status, limit, children) != (0, None, False):
                raise h.HarnessError(f'cannot compile original catalog probe: {out!r} {err!r}')
            for name, chain, c_locale in rows:
                translation = translator(paths, chain, c_locale)
                runs = []
                reset = root / 'reset'
                for executable in (args.oracle, args.oracle, args.candidate):
                    try:
                        runs.append(observe(executable.absolute(), root / 'make', reset, probe,
                                            locale, chain, c_locale, translation))
                    finally:
                        h.remove_tree(reset)
                passed = not runs[0]['errors'] and runs[0] == runs[1] == runs[2]
                print('PASS' if passed else 'FAIL', name, [run['errors'] for run in runs], flush=True)
                results.append({'id': name, 'language': chain, 'pass': passed, 'runs': runs})
    finally:
        os.umask(previous_umask)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    passed = sum(row['pass'] for row in results)
    args.report.write_text(json.dumps({'passed': passed, 'cases': len(results),
                                      'oracle_profile': reference['profile_id'], 'results': results,
                                      'candidate_sha256': h.sha256_file(args.candidate)}, indent=2) + '\n')
    print(f'Catalog coverage gate: {passed}/{len(results)}', flush=True)
    return int(passed != len(results))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (h.HarnessError, oracle.OracleError, OSError, ValueError) as error:
        raise SystemExit(f'catalog coverage gate: {error}') from error
