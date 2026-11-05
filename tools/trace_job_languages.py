#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Check original trace and serial JOBS models in three additional languages."""
import argparse
from dataclasses import replace
import gettext
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile

import conformance as h
import job_debug as jobs
import nls_diagnostics as nls
import trace_nls as trace

CATALOGS = {
    'de': '89caeb7cd1950ece3af7b387ecb4939217c0763a9601a22d10e9b311006789e3',
    'ja': 'd70ac36466b5a450077004349818a4a2f0dcaf699e7caca70822b95307bdbe49',
    'pt_BR': 'a42a7c6f01d2a43b01567beeb6784b28b3a9048ea7ddf741ef89994ca5eb4f72',
}
TRACE_CASES = ('trace-phony-parallel-sync', 'trace-missing-dry',
               'trace-existing-newer-duplicates-normal', 'trace-builtin')
JOB_CASES = ('jobs-serial-multiline', 'jobs-ignored-failure')
JOB_FORMATS = {
    'put': 'Putting child %p (%s) PID %s%s on the chain.\n',
    'live': 'Live child %p (%s) PID %s %s\n',
    'win': 'Reaping winning child %p PID %s %s\n',
    'lose': 'Reaping losing child %p PID %s %s\n',
    'remove': 'Removing child %p PID %s%s from chain.\n',
}
ADDRESS = b'<declared-job-address>'
# Bind the stream declarations to the exact independent GNU2 qualification.
STREAM_PLAN_SHA256 = '35a9a2a8e7ea3906030fe2060adda2a93551cdbed9d9d74ab1000d3c30b3b2a2'


def translations(reference_french):
    installed = reference_french.parents[2]
    for language, digest in CATALOGS.items():
        name = Path(language) / 'LC_MESSAGES/make.mo'
        path = installed / name
        if h.sha256_file(path) != digest or h.sha256_file(trace.ROOT / 'catalogs' / name) != digest:
            raise h.HarnessError('localized trace/JOBS catalog identity differs: ' + language)
        with path.open('rb') as stream:
            yield language, gettext.GNUTranslations(stream)


def local_job_stderr(model, catalog):
    if model.stderr == b'':
        return b''
    if model.stderr != b'make: [Makefile:3: all] Error 3 (ignored)\n':
        raise h.HarnessError('unexpected original JOBS error model')
    ignored = catalog.gettext(' (ignored)').encode()
    template = catalog.gettext('%s[%s: %s] Error %d%s%s').encode()
    return b'make: ' + template % (b'', b'Makefile:3', b'all', 3, ignored, b'') + b'\n'


def declarations(root, runtime, reference, catalog_path, locale):
    selected_trace = [model for model in trace.models(root) if model.case.id in TRACE_CASES]
    selected_jobs = [model for model in jobs.models() if model.id in JOB_CASES]
    if len(selected_trace) != len(TRACE_CASES) or len(selected_jobs) != len(JOB_CASES):
        raise h.HarnessError('missing original trace/JOBS model')
    for language, catalog in translations(catalog_path):
        environment = {**locale['environment'], 'LANG': locale['locale'], 'LC_ALL': locale['locale'],
                       'LANGUAGE': language}
        for model in selected_trace:
            case = trace.localized(model, lambda key: catalog.gettext(key).encode(), environment)
            stream = {'id': case.id + '-' + language, 'expected': case.expected}
            filesystem = trace.prepare(model, case, runtime, reference)
            yield model, case, stream, filesystem
        for original in selected_jobs:
            if original.style or any(op not in JOB_FORMATS for op, _, _ in original.events):
                raise h.HarnessError('the localized JOBS model must be serial')
            model = replace(original, stderr=local_job_stderr(original, catalog))
            fixture = root / (model.id + '-' + language)
            fixture.mkdir()
            for name, data in {'Makefile': model.source, 'worker.py': jobs.WORKER.read_bytes(), 'seen': b'anchor\n'}.items():
                (fixture / name).write_bytes(data)
            filesystem = h.snapshot({'work': fixture})
            filesystem.update({name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp')})
            case = h.Case(fixture, fixture.name, 'debug.jobs', 'required', ('-rR', '--no-print-directory', '--debug=j'),
                          6, {'GNUMAKEFLAGS': '', **environment}, {})
            stream = {'id': case.id, 'expected_events': model.events, 'stderr_hex': model.stderr.hex(), 'status': model.status,
                      'templates_hex': {op: catalog.gettext(text).encode().hex() for op, text in JOB_FORMATS.items()}}
            yield model, case, stream, filesystem


def job_line(template, address, operation, target, pid):
    if template.count(b'%p') != 1:
        raise h.HarnessError('JOBS format requires one address field')
    args = (address, target, pid, b'') if operation in ('put', 'live') else (address, pid, b'')
    return template.replace(b'%p', b'%s') % args


def job_errors(model, observed, host, runtime, initial, templates):
    lines = observed.stdout.splitlines(keepends=True)
    errors = []
    original_events = []
    if len(lines) != len(model.events):
        errors.append('localized job event count')
    for index, (line, (op, target, worker)) in enumerate(zip(lines, model.events)):
        pid = str(host['workers'].get(worker, {}).get('pid', -1)).encode()
        args = (op, target.encode(), pid)
        prefix, suffix = job_line(templates[op], ADDRESS, *args).split(ADDRESS)
        match = re.fullmatch(re.escape(prefix) + rb'(0x[0-9a-f]+)' + re.escape(suffix), line)
        if match is None:
            errors.append(f'localized event {index}: exact bytes and witnessed PID')
        else:
            original_events.append(job_line(JOB_FORMATS[op].encode(), match[1], *args))
    # Every localized byte and PID was checked above. Reuse the original
    # address-lifetime, live witness, stderr, status and filesystem validation.
    return errors + jobs.assess(model, replace(observed, stdout=b''.join(original_events)), host, runtime, initial)


def run(model, case, stream, filesystem, runtime, binary):
    if isinstance(model, trace.Model):
        observed = h.execute(case, runtime, binary)
        errors = h.expectation_errors(case, observed)
        if observed.filesystem != filesystem:
            errors.append('independent complete filesystem')
        details = {'mtime_ns': {}}
        for name, stamp in model.mtimes.items():
            path = runtime / 'work' / name
            actual = path.stat().st_mtime_ns if path.is_file() else None
            details['mtime_ns'][name] = actual
            if actual != stamp * 10**9:
                errors.append('fixed input mtime: ' + name)
    else:
        observed, host = jobs.execute(model, case, runtime, binary)
        templates = {op: bytes.fromhex(text) for op, text in stream['templates_hex'].items()}
        errors = job_errors(model, observed, host, runtime, filesystem, templates)
        details = {'host': host}
    return {**h.observation_json(observed), 'stdout_hex': observed.stdout.hex(), 'stderr_hex': observed.stderr.hex(),
            **details, 'errors': errors}, bool(observed.limit or observed.leftover_processes or observed.returncode < 0)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--locale-root', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    reference, profile, catalog_path, locale = nls.prepare_reference(args.oracle, trace.CATALOG_HASH, args.locale_root)
    binaries = (reference, reference, args.candidate.resolve())
    report = {'classification': 'Strict original non-French trace/JOBS models', 'oracle_profile': profile['profile_id'],
              'locale_profile': locale, 'catalog_sha256': CATALOGS, 'stream_plan_sha256': STREAM_PLAN_SHA256,
              'functional_cpu_affinity': sorted(os.sched_getaffinity(0)),
              'binaries': [h.executable_identity(binary, oracle=index < 2) for index, binary in enumerate(binaries)],
              'source_hashes': {name: h.sha256_file(Path(path)) for name, path in
                                {'driver': __file__, 'trace': trace.__file__, 'jobs': jobs.__file__,
                                 'conformance': h.__file__, 'worker': jobs.WORKER}.items()}, 'cases': []}
    with tempfile.TemporaryDirectory(prefix='rmake-trace-job-languages-') as directory:
        root = Path(directory)
        runtime = root / 'runtime'
        runtime.mkdir()
        selected = list(declarations(root, runtime, reference, catalog_path, locale))
        plan = [stream for _, _, stream, _ in selected]
        digest = hashlib.sha256(json.dumps(plan, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
        if len(plan) != 18 or digest != STREAM_PLAN_SHA256:
            raise h.HarnessError('original localized trace/JOBS stream declaration changed')
        for model, case, stream, filesystem in selected:
            row = {**stream, 'args': case.args, 'environment': case.environment, 'filesystem_model': filesystem,
                   'observations': []}
            bounded = False
            for binary in binaries:
                observation, bounded = run(model, case, stream, filesystem, runtime, binary)
                row['observations'].append(observation)
                if bounded:
                    break
            row['passed'] = not bounded and all(not observed['errors'] for observed in row['observations'])
            report['cases'].append(row)
            print('PASS' if row['passed'] else 'FAIL', row['id'], [run['errors'] for run in row['observations']], flush=True)
            if bounded:
                break
    passed = sum(row['passed'] for row in report['cases'])
    report['totals'] = {'cases': len(plan), 'pass': passed}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return int(passed != len(plan))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (h.HarnessError, OSError, ValueError) as error:
        raise SystemExit(f'localized trace/JOBS gate: {error}') from error
