#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

"""Check exact C and French databases with existing independent witnesses."""
import argparse
from dataclasses import replace
import gettext
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import tempfile

import conformance as h
import database_output as db
import locale_data
import nls_diagnostics as nls
import oracle as reference

ROOT = Path(__file__).resolve().parents[1]
LOCALE_FIXTURE = ROOT / 'tests/database-output/nls_preload.c'

REFERENCE_MO_SHA256 = 'dd9f6c8d75cf94c0f4a01e4ad3ee2b833b4b552f13d0adf461b83a1356ed17a4'
CASES = (
    'database-minimal', 'database-target-pattern-scope', 'database-pattern-rules',
    'database-vpath-search', 'database-read-fatal', 'database-phase-cli-eval-fatal',
    'database-phase-restart', 'database-variable-forms',
)


def make_catalog(path):
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != REFERENCE_MO_SHA256:
        raise ValueError('unexpected original French catalog')
    with path.open('rb') as source:
        return gettext.GNUTranslations(source)


def format_message(template, *fields):
    # The pinned compiled catalog uses these ordinary printf types only in
    # this fixture; preserve argument data as bytes, never run a native printf.
    fields = iter(fields)
    tokens = re.compile(rb'%%|%(?:[hl]?[du]|\.([01])f|s)')
    result = tokens.sub(lambda m: b'%' if m[0] == b'%%' else next(fields), template)
    if next(fields, None) is not None:
        raise ValueError('wrong catalog argument count')
    return result


def locale_transcript(transcript, catalog, *, first_dump_only, decimal_comma=False):
    def tr(message, *fields):
        return format_message(catalog.gettext(message).encode(), *fields)

    boundaries = {
        b'# Variables\n': ('\n# Variables\n', b''),
        b'# Pattern-specific Variable Values\n': ('\n# Pattern-specific Variable Values', b'\n'),
        b'# No pattern-specific variable values.\n': ('\n# No pattern-specific variable values.', b'\n'),
        b'# Directories\n': ('\n# Directories\n', b''),
        b'# Implicit Rules\n': ('\n# Implicit Rules', b'\n'),
        b'# No implicit rules.\n': ('\n# No implicit rules.', b'\n'),
        b'# Files\n': ('\n# Files', b'\n'),
        b'# VPATH Search Paths\n': ('\n# VPATH Search Paths\n', b''),
        b'# files hash-table stats:\n': ('\n# files hash-table stats:\n# ', b''),
        b'# hash-table stats:\n': ('# hash-table stats:\n# ', b''),
        b'# variable set hash-table stats:\n': ('# variable set hash-table stats:\n', b''),
        b'# No \'vpath\' search paths.\n': ("# No 'vpath' search paths.", b'\n'),
        b'# General (\'VPATH\' variable) search path:\n': ("\n# General ('VPATH' variable) search path:\n# ", b''),
        b'# No general (\'VPATH\' variable) search path.\n': ("\n# No general ('VPATH' variable) search path.", b'\n'),
        b'# No strcache buffers\n': ('\n%s No strcache buffers\n', b''),
    }
    literal_comments = {
        b'# Not a target:', b'#  Precious file (prerequisite of .PRECIOUS).',
        b'#  Phony target (prerequisite of .PHONY).', b'#  Command line target.',
        b'#  A default, MAKEFILES, or -include/sinclude makefile.', b'#  Builtin rule',
        b'#  Implicit rule search has been done.', b'#  Implicit rule search has not been done.',
        b'#  File is an intermediate prerequisite.', b'#  File is a prerequisite of .NOTINTERMEDIATE.',
        b'#  File is secondary (prerequisite of .SECONDARY).', b'#  Modification time never checked.',
        b'#  File does not exist.', b'#  File is very old.',
        b'#  File has been updated.', b'#  File has not been updated.',
        b'#  Successfully updated.', b'#  Needs to be updated (-q is set).',
        b'#  Failed to be updated.',
    }
    header_license = (
        b'# License GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>\n'
        b'# This is free software: you are free to change and redistribute it.\n'
        b'# There is NO WARRANTY, to the extent permitted by law.\n'
    )
    header_msg = header_license.decode().replace('# ', '%s')
    heading = re.compile(rb'# (Make data base, printed on |Finished Make data base on )(@CTIME_[A-Z_0-9]+@)\n')
    origin = re.compile(rb"# (automatic|default|environment under -e|environment|makefile|command line|'override' directive)( private)?(?: \(from '(.*)', line ([0-9]+)\))?\n")
    recipe = re.compile(rb"#  recipe to execute \(from '(.*)', line ([0-9]+)\):\n")
    hashstats = re.compile(rb'# Load=([0-9]+)/([0-9]+)=([0-9]+)%, Rehash=([0-9]+), Collisions=([0-9]+)/([0-9]+)=([0-9]+)%\n')
    dir_row = re.compile(rb'# (.+) \(device (@DEVICE_[0-9]+@), inode (@INODE_[0-9]+@)\): (.*)\n')
    dir_count = re.compile(rb'(No|[0-9]+) files, (no|[0-9]+) impossibilities( so far| in [0-9]+ directories)?\.')
    fields = [
        (rb'# ([0-9]+) pattern-specific variable values\n', '\n# %u pattern-specific variable values', b'\n'),
        (rb'# ([0-9]+) implicit rules, ([0-9]+) \(([0-9]+\.[0-9])%\) terminal\.\n', '\n# %u implicit rules, %u (%.1f%%) terminal.', b'\n'),
        (rb"# ([0-9]+) 'vpath' search paths\.\n", "\n# %u 'vpath' search paths.\n", b''),
        (rb"#  Implicit/static pattern stem: '(.*)'\n", "#  Implicit/static pattern stem: '%s'\n", b''),
        (rb'#  Last modified (.*)\n', '#  Last modified %s\n', b''),
        (rb'# strcache buffers: ([0-9]+) \(([0-9]+)\) / strings = ([0-9]+) / storage = ([0-9]+) B / avg = ([0-9]+) B\n', '\n%s strcache buffers: %lu (%lu) / strings = %lu / storage = %lu B / avg = %lu B\n', b''),
        (rb'# current buf: size = ([0-9]+) B / used = ([0-9]+) B / count = ([0-9]+) / avg = ([0-9]+) B\n', '%s current buf: size = %hu B / used = %hu B / count = %hu / avg = %u B\n', b''),
        (rb'# strcache performance: lookups = ([0-9]+) / hit rate = ([0-9]+)%\n', '\n%s strcache performance: lookups = %lu / hit rate = %lu%%\n', b''),
        (rb'# other used: total = ([0-9]+) B / count = ([0-9]+) / avg = ([0-9]+) B\n', '%s other used: total = %lu B / count = %lu / avg = %lu B\n', b''),
        (rb'# other free: total = ([0-9]+) B / max = ([0-9]+) B / min = ([0-9]+) B / avg = ([0-9]+) B\n', '%s other free: total = %lu B / max = %lu B / min = %lu B / avg = %hu B\n', b''),
    ]
    fields = [(re.compile(pattern), message, suffix) for pattern, message, suffix in fields]

    def counts(value):
        match = dir_count.fullmatch(value)
        if match is None:
            raise ValueError('unexpected modeled directory count grammar')
        files, impossible, tail = match.groups()
        start = (tr('No') if files == b'No' else files) + tr(' files, ')
        start += tr('no') if impossible == b'no' else impossible
        if tail and tail.startswith(b' in '):
            return start + tr(' impossibilities in %lu directories.\n', tail.split(b' ')[2])
        return start + tr(' impossibilities') + (tr(' so far.') if tail else b'.') + b'\n'

    def translate(line):
        for key in ("'%s' is up to date.", "Nothing to be done for '%s'."):
            prefix, suffix = key.encode().split(b'%s')
            if line.startswith(b'make: ' + prefix) and line.endswith(suffix + b'\n'):
                name = line[len(b'make: ') + len(prefix):-len(suffix) - 1]
                return b'make: ' + tr(key, name) + b'\n'
        if (match := heading.fullmatch(line)) is not None:
            start, slot = match.groups()
            message = '\n# ' + start.decode() + '%s'
            if start.startswith(b'Finished'):
                return tr(message + '\n', slot + b'\n')[1:-1]
            return tr(message, slot + b'\n')[1:]
        if line.startswith(b'# Built for '):
            return tr('%sBuilt for %s\n', b'# ', line.removeprefix(b'# Built for ').removesuffix(b'\n'))
        if line in boundaries:
            message, suffix = boundaries[line]
            value = tr(message, b'#') if '%s' in message else tr(message)
            return value.removeprefix(b'\n').removesuffix(b'\n# ') + (b'\n' if value.endswith(b'\n# ') else b'') + suffix
        if line.removesuffix(b'\n') in literal_comments:
            return tr(line[:-1].decode()) + b'\n'
        if (match := origin.fullmatch(line)) is not None:
            label, private, source, number = match.groups()
            return b'# ' + tr(label.decode()) + (private or b'') + (
                tr(" (from '%s', line %lu)", source, number) if source is not None else b'') + b'\n'
        if (match := recipe.fullmatch(line)) is not None:
            return tr('#  recipe to execute') + tr(" (from '%s', line %lu):\n", *match.groups())
        if line == b'#  recipe to execute (built-in):\n':
            return tr('#  recipe to execute') + tr(' (built-in):') + b'\n'
        if (match := hashstats.fullmatch(line)) is not None:
            numbers = match.groups()
            return b'# ' + tr('Load=%lu/%lu=%.0f%%, ', *numbers[:3]) + tr('Rehash=%u, ', numbers[3]) + tr('Collisions=%lu/%lu=%.0f%%', *numbers[4:]) + b'\n'
        if (match := dir_row.fullmatch(line)) is not None:
            path, device, inode, values = match.groups()
            return tr('# %s (device %ld, inode %ld): ', path, device, inode) + counts(values)
        if line.startswith(b'# ') and dir_count.fullmatch(line[2:-1]):
            return b'# ' + counts(line[2:-1])
        for pattern, message, suffix in fields:
            if (match := pattern.fullmatch(line)) is not None:
                prefix = (b'#',) if message.lstrip().startswith('%s') else ()
                groups = match.groups()
                if decimal_comma and '%.1f' in message:
                    count, terminal, percent = groups
                    groups = (count, terminal, percent.replace(b'.', b','))
                return tr(message, *prefix, *groups).removeprefix(b'\n') + suffix
        return line

    if first_dump_only:
        marker = b'# Finished Make data base on @CTIME_CLOSE_0@\n'
        end = transcript.index(marker) + len(marker)
        locale_part, tail = transcript[:end], transcript[end:]
    else:
        locale_part, tail = transcript, b''
    license_placeholder = b'#\0LICENSE\0\n'
    heading_start = locale_part.index(b'\n# Make data base, printed on ')
    header, body = locale_part[:heading_start], locale_part[heading_start:]
    if header.count(header_license) != 1:
        raise ValueError('expected exactly one version license block before the database')
    locale_part = header.replace(header_license, license_placeholder, 1) + body
    rows = []
    in_definition = False
    for line in locale_part.splitlines(keepends=True):
        if line.startswith(b'define '):
            in_definition = True
        if in_definition:
            rows.append(line)
            if line == b'endef\n':
                in_definition = False
        elif line == license_placeholder:
            rows.append(tr(header_msg, b'# ', b'# ', b'# '))
        else:
            rows.append(translate(line))
    if in_definition:
        raise ValueError('unterminated modeled multiline variable')
    return b''.join(rows) + tail


def with_payload(model):
    if model.id != 'database-variable-forms':
        return model
    original = b'define DB_MULTI\none\ntwo\nendef'
    payload = b'define DB_MULTI\n# Files\nliteral-\xff\nendef'
    source = model.files['Makefile']
    if source.count(original) != 1 or model.stdout.count(original) != 1:
        raise ValueError('raw database payload fixture no longer has one declared site')
    return replace(model, files={**model.files, 'Makefile': source.replace(original, payload)},
                   stdout=model.stdout.replace(original, payload),
                   required=tuple(item.replace(original, payload) for item in model.required))


def localized(original, catalog, language):
    stdout = locale_transcript(original.stdout, catalog, first_dump_only=original.dump_count > 1,
                               decimal_comma=language == 'fr-numeric')
    stderr = original.stderr.replace(
        b'.  Stop.\n', catalog.gettext('.  Stop.\n').encode()
    )
    # These are independent fields of the original model, not observations.
    # Literal full expected output still checks every translated byte, and
    # raw payload assertions remain unchanged when present in the original.
    required = tuple(item for item in original.required if item in stdout)
    return replace(original, stdout=stdout, stderr=stderr, required=required, sections=())


def publish(path, report):
    with path.open('x') as output:
        json.dump(report, output, indent=2)
        output.write('\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--locale-root', type=Path, default=ROOT / '.tools/locale-data')
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--expected', type=Path)
    parser.add_argument('--case', action='append', default=[])
    args = parser.parse_args()
    oracle, oracle_profile, catalog_path, locale = nls.prepare_reference(
        args.oracle, REFERENCE_MO_SHA256, args.locale_root)
    catalog = make_catalog(catalog_path)
    selected = [case for case in CASES if not args.case or case in args.case]
    if not selected or set(args.case) - set(selected):
        parser.error('unknown or empty original case selection')
    catalog_hashes = {path.name: h.sha256_file(path) for path in (
        LOCALE_FIXTURE, Path(__file__), catalog_path)}
    identities = {'oracle': h.executable_identity(oracle, oracle=True)}
    if args.candidate:
        identities['candidate'] = h.executable_identity(args.candidate)
    report = {
        'classification': 'Exact original database locale differential with independent public-input and real libc time witnesses',
        'oracle_profile_id': oracle_profile['profile_id'], 'identities': identities,
        'locale_profile': locale, 'source_sha256': catalog_hashes,
        'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
        'expected_source_hashes': db.expected_source_hashes(), 'records': [], 'passed': False,
    }
    known = {model.id: model for model in db.models('all')}
    if args.expected and args.expected.exists():
        parser.error('pre-observation expectations output must be new')
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='rmake-database-nls-') as temporary:
        root = Path(temporary).resolve()
        runtime = root / 'runtime'
        runtime.mkdir()
        preload = root / 'messages.so'
        locpath = locale['environment']['LOCPATH']
        compiler = shutil.which('cc')
        if compiler is None:
            raise h.HarnessError('database locale fixture requires cc')
        command = [compiler, '-std=c11', '-fPIC', '-shared', '-O2', '-Wall', '-Wextra',
                   '-Werror', '-DPROFILE_LOCPATH=' + json.dumps(locpath),
                   str(LOCALE_FIXTURE), '-o', str(preload)]
        out, err, code, limit, leftovers = h.capture(command, root, h.clean_environment(root), 30)
        if code != 0 or limit or leftovers:
            raise h.HarnessError('original database locale fixture failed compilation: ' + repr(err))
        report['compilation'] = {'argv': command, 'sha256': h.sha256_file(preload),
                                 'stdout_hex': out.hex(), 'stderr_hex': err.hex()}
        preparation = db.expected_preparation(oracle_profile, root)
        declarations = []
        for selection in selected:
            original = known[selection]
            languages = ('C', 'fr', 'fr-numeric') if selection == 'database-pattern-rules' else ('C', 'fr')
            for language in languages:
                with_data = with_payload(original)
                model = with_data if language == 'C' else localized(with_data, catalog, language)
                case, filesystem = db.prepare(model, root / f'{selection}-{language}', runtime)
                case = replace(case, environment={**case.environment, 'LD_PRELOAD': str(preload),
                                                 'RMAKE_TEST_DB_MESSAGES': language})
                record = {'id': f'{selection}-{language}', 'base_case': selection, 'language': language,
                          'args': case.args, 'environment': case.environment,
                          'declared_files_hex': {name: contents.hex() for name, contents in model.files.items()},
                          'expected_full_filesystem': filesystem, 'expected_status': model.expected_status,
                          'expected_stderr_hex': model.stderr.hex(), 'declared_dump_count': model.dump_count,
                          'directory_slots': model.directories, 'expected_seen_hex': model.seen.hex(),
                          'expected_stdout_template_hex': model.stdout.hex()}
                # Check every model/clock slot before launching GNU.
                db.compile_template(model, runtime)
                declarations.append((original, case, filesystem, language, record))
        if args.expected:
            publish(args.expected, {
                'locale_fixture_sha256': h.sha256_file(preload), 'sources': catalog_hashes,
                'runtime_path': str(runtime), 'records': [item[-1] for item in declarations],
            })
            report['expectations_sha256'] = h.sha256_file(args.expected)
        report['runtime_path'] = str(runtime)
        terminate = False
        for original, case, filesystem, language, record in declarations:
            row = dict(record, observations=[], passed=False)
            report['records'].append(row)
            for executable in [oracle, oracle, *([args.candidate] if args.candidate else [])]:
                execution = db.execute(original, case, runtime, executable, expected=preparation)
                prepared = with_payload(db.prepare_expected_model(original, execution))
                if language != 'C':
                    prepared = localized(prepared, catalog, language)
                errors = db.prepared_stream_errors(prepared, runtime, execution)
                actual = execution.observation
                row['observations'].append(dict(db.evidence(execution), expected_errors=errors,
                                                prepared_stdout_template_hex=prepared.stdout.hex(),
                                                filesystem_matches=actual.filesystem == filesystem,
                                                independent_expected_inputs=execution.expected_inputs.evidence()))
                if actual.limit or actual.leftover_processes or actual.returncode < 0:
                    row['zero_credit'] = True
                    terminate = True
                    break
            row['passed'] = not terminate and all(
                not item['expected_errors'] and item['filesystem_matches']
                for item in row['observations'])
            print(('PASS ' if row['passed'] else 'FAIL ') + row['id'], flush=True)
            if terminate:
                break
    report['passed'] = all(row['passed'] for row in report['records'])
    report['totals'] = {'cases': len(report['records']),
                        'pass': sum(row['passed'] for row in report['records'])}
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, h.HarnessError, locale_data.ProfileError, reference.OracleError) as error:
        raise SystemExit(str(error)) from error
