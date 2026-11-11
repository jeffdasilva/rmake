#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Candidate-only safe-error gate. These are exceptions, never oracle matches.

Known GNU crashes/hangs are retained as immutable research evidence. This gate
does not invoke GNU or repeat those faults. Inputs are reviewed trusted fixtures;
the existing process-group/resource containment is not a security sandbox.
"""
from __future__ import annotations

import argparse
import dataclasses
import hashlib
import json
from pathlib import Path
import re
import tempfile

import conformance as harness

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'tests/safety/manifest.json'
CODES = {'S001', 'S002', 'S003', 'S004', 'S005', 'S006'}
NORMAL_SHORT_APPROVAL = 'nested-short-state-2026-11-02'
NORMAL_SHORT_SCOPE = ({'flag-payload-ordered-14'}
    | {f'nested-cursor-tail-{count}-{kind}-short' for count in range(2, 7)
       for kind in ('simple', 'recursive')}
    | {f'nested-boundary-{kind}-p1-{word}' for kind in ('simple', 'recursive')
       for word in ('short', 'long')})


def validate_normal_approval(approval, records):
    """Validate the explicit owner's finite scope; never waive normal inputs generally."""
    if (approval.get('schema') != 1 or approval.get('id') != NORMAL_SHORT_APPROVAL
            or approval.get('approved') is not True or approval.get('code') != 'S004'
            or approval.get('owner_decision') != 'Extend documented S004 errors (Recommended)'):
        raise harness.HarnessError('normal-exit safety scope lacks explicit owner approval')
    scope = {item['id']: item for item in approval['scope']}
    if len(scope) != len(approval['scope']) or set(scope) != NORMAL_SHORT_SCOPE:
        raise harness.HarnessError('normal-exit approval differs from the 15 approved inputs')
    by_id = {item['id']: item for item in records}
    if len(by_id) != len(records) or set(by_id) != NORMAL_SHORT_SCOPE:
        raise harness.HarnessError('normal-exit evidence differs from approved scope')
    for name, record in by_id.items():
        runs = record.get('oracle_runs', [])
        if len(runs) < 2 or not all(run.get('status') == 0 and not run.get('limit')
                and not run.get('leftovers') for run in runs):
            raise harness.HarnessError(f'{name}: normal-exit approval needs bounded status-0 evidence')
        expected = scope[name]
        inputs = {file: hashlib.sha256(bytes.fromhex(value)).hexdigest()
                  for file, value in evidence_inputs(record).items()}
        if (expected['inputs'] != inputs or expected['args'] != record['args']
                or expected['environment'] != record['environment']):
            raise harness.HarnessError(f'{name}: approved invocation/input evidence changed')
    unstable = approval.get('instability_evidence', [])
    required = {'nested-cursor-tail-4-recursive-short', 'nested-cursor-tail-5-simple-short',
                'nested-cursor-tail-5-recursive-short', 'nested-boundary-simple-p1-short'}
    if len(unstable) != 4 or set(unstable) != required:
        raise harness.HarnessError('missing explicit GNU policy-instability evidence')
    for name in unstable:
        signatures = set()
        for run in by_id[name]['oracle_runs']:
            stdout = bytes.fromhex(run['stdout'])
            flags = re.search(rb'FLAGS=\[([^]]*)\]', stdout)
            if flags is None:
                raise harness.HarnessError(f'{name}: missing policy snapshot')
            signatures.add((flags[1], b'GNU Make 4.4.1\n' in stdout))
        if len(signatures) < 2:
            raise harness.HarnessError(f'{name}: PID/address differences do not establish a policy hazard')
    return scope


def normal_approval(ledger):
    reference = ledger.get('normal_exit_approval')
    if reference is None:
        return {}, None
    path = ROOT / reference['path']
    if path != ROOT / 'tests/safety/nested-short-normal-approval.json' or harness.sha256_file(path) != reference['sha256']:
        raise harness.HarnessError('normal-exit approval path/hash changed')
    approval = json.loads(path.read_text())
    evidence = ROOT / approval['evidence']
    if evidence != ROOT / 'tests/research/gnu-441-nested-short-state.json' or harness.sha256_file(evidence) != approval['evidence_sha256']:
        raise harness.HarnessError('normal-exit approval evidence path/hash changed')
    records = json.loads(evidence.read_text())['records']
    return validate_normal_approval(approval, records), approval


def evidence_inputs(record):
    files = dict(record.get('inputs_hex', record.get('input_files', {})))
    if 'makefile_hex' in record:
        files['Makefile'] = record['makefile_hex']
    if 'capture_script_hex' in record:
        files['capture'] = record['capture_script_hex']
    return files


def inventory(manifest=MANIFEST):
    ledger = json.loads(manifest.read_text())
    if ledger.get('schema') != 1:
        raise harness.HarnessError('unsupported safety manifest schema')
    normal_scope, approval = normal_approval(ledger)
    cases = []
    seen = set()
    normal_seen = set()
    for row in ledger['exceptions']:
        name = row['id']
        if name in seen or not name or Path(name).name != name or name in ('.', '..'):
            raise harness.HarnessError('invalid or duplicate safety id')
        seen.add(name)
        if row['code'] not in CODES:
            raise harness.HarnessError(f'{name}: unknown safety code')
        evidence = ROOT / row['evidence']
        if evidence.parent != ROOT / 'tests/research':
            raise harness.HarnessError(f'{name}: evidence must be a research record')
        if harness.sha256_file(evidence) != row['evidence_sha256']:
            raise harness.HarnessError(f'{name}: research evidence changed')
        records = json.loads(evidence.read_text())['records']
        matches = [record for record in records if record['id'] == name]
        if len(matches) != 1:
            raise harness.HarnessError(f'{name}: missing or duplicate research record')
        record = matches[0]
        runs = record.get('oracle_runs', [record])
        if row.get('approval') is not None:
            if (row['approval'] != NORMAL_SHORT_APPROVAL or name not in normal_scope
                    or row['code'] != 'S004' or row['evidence'] != approval['evidence']
                    or row['evidence_sha256'] != approval['evidence_sha256']
                    or normal_scope[name]['inputs'] != row['inputs']
                    or normal_scope[name]['args'] != row['args']
                    or normal_scope[name]['environment'] != row['environment']):
                raise harness.HarnessError(f'{name}: normal-exit exception is outside approved scope')
            normal_seen.add(name)
        elif not runs or not all(run.get('status', run.get('exit_status',
                run.get('observed_exit_status', 0))) < 0 or run.get('limit') for run in runs):
            raise harness.HarnessError(f'{name}: evidence does not record an oracle fault')
        if row['args'] != record.get('args', record.get('argv')) or row['environment'] != record.get('environment', {}):
            raise harness.HarnessError(f'{name}: invocation differs from research evidence')
        fixture = ROOT / 'tests/safety/cases' / name
        harness.validate_fixture(fixture)
        inputs = evidence_inputs(record)
        if set(inputs) != set(row['inputs']) or set(inputs) != {p.name for p in fixture.iterdir()}:
            raise harness.HarnessError(f'{name}: fixture inventory differs from evidence')
        for filename, value in inputs.items():
            path = fixture / filename
            if path.read_bytes() != bytes.fromhex(value) or harness.sha256_file(path) != row['inputs'][filename]:
                raise harness.HarnessError(f'{name}: fixture bytes differ from evidence')
        if 'capture_mode' in record and (fixture / 'capture').stat().st_mode & 0o777 != int(record['capture_mode'], 8):
            raise harness.HarnessError(f'{name}: observer mode differs from evidence')
        expected = row['expected']
        marker = f'rmake safety error [{row["code"]}]'.encode()
        if set(expected) != {'exit_code', 'stdout_hex', 'stderr_hex'} or expected['exit_code'] != 2 or marker not in bytes.fromhex(expected['stderr_hex']):
            raise harness.HarnessError(f'{name}: a normal fatal error with a safety code is required')
        cases.append(harness.Case(fixture, name, 'safety.' + row['code'],
            'required', tuple(row['args']), 5, row['environment'], expected))
    if normal_seen != set(normal_scope):
        raise harness.HarnessError('approved normal-exit scope is not fully mapped')
    if seen != {p.name for p in (ROOT / 'tests/safety/cases').iterdir()}:
        raise harness.HarnessError('unmapped safety fixture')
    if seen & {p.name for p in (ROOT / 'tests/cases').iterdir()}:
        raise harness.HarnessError('safety exceptions must not be ordinary conformance cases')
    return cases


def errors(case, observation):
    # Reuse the independent exact stream/status anchors and containment checks.
    failures = [message.replace('oracle', 'candidate')
                for message in harness.expectation_errors(case, observation)]
    inputs = input_filesystem(case)
    allowed = set(inputs)
    if case.requirement == 'safety.S003':
        # These original inputs compile their owned observer before the hazard.
        # Its bytes depend on the host compiler; the input C source stays exact.
        allowed.add('work/capture')
        compiled = observation.filesystem.get('work/capture', {})
        if compiled.get('type') != 'file' or not compiled.get('mode', 0) & 0o111:
            failures.append('missing compiled pre-hazard observer')
    if set(observation.filesystem) != allowed:
        failures.append('unexpected or missing filesystem effects')
    for name, expected in inputs.items():
        if observation.filesystem.get(name) != expected:
            failures.append(f'input or workspace changed: {name}')
    return failures


def input_filesystem(case):
    return dict(harness.snapshot({'work': case.path}),
                home={'type': 'directory', 'mode': 0o755},
                tmp={'type': 'directory', 'mode': 0o755})


def raw(observation):
    return dict(harness.observation_json(observation),
                stdout_hex=observation.stdout.hex(), stderr_hex=observation.stderr.hex())


def variants(case):
    # Ignore/keep-going and local parallel execution must not suppress a fatal
    # safety error. Count these cells separately from the exception inventory.
    for label, args in (('normal', ()), ('ignore', ('-i',)),
                        ('keep-going', ('-k',)), ('parallel', ('-j4',))):
        # Keep-going lets GNU visit the otherwise blocked double-colon tail.
        if case.requirement == 'safety.S006' and label == 'keep-going':
            continue
        yield label, dataclasses.replace(case, args=case.args + args)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidate', required=True, type=Path)
    parser.add_argument('--repeat', type=int, default=2)
    parser.add_argument('--artifacts', type=Path, default=ROOT / 'artifacts/safety')
    args = parser.parse_args(argv)
    if args.repeat < 2:
        parser.error('at least two observations are required for deterministic safe errors')
    try:
        cases = inventory()
        identity = harness.executable_identity(args.candidate)
        args.artifacts.mkdir(parents=True, exist_ok=True)
        destination = Path(tempfile.mkdtemp(prefix='exceptions-', dir=args.artifacts))
        report = dict(schema=1, classification='safety exceptions; not GNU conformance',
                      candidate=identity, manifest_sha256=harness.sha256_file(MANIFEST), cases=[])
        failed = 0
        cells = 0
        with tempfile.TemporaryDirectory(prefix='rmake-safety-') as temporary:
            for case in cases:
                for label, cell in variants(case):
                    cells += 1
                    observations = [harness.execute(cell, Path(temporary), args.candidate.resolve())
                                    for _ in range(args.repeat)]
                    failures = [message for observation in observations for message in errors(cell, observation)]
                    if any(observation != observations[0] for observation in observations[1:]):
                        failures.append('candidate observations are not deterministic')
                    failed += bool(failures)
                    report['cases'].append(dict(id=case.id, variant=label, code=case.requirement,
                        argv=list(cell.args), errors=failures,
                        observations=[raw(observation) for observation in observations]))
                    print(('FAIL ' if failures else 'SAFE ') + case.id + '/' + label, flush=True)
        report['totals'] = dict(exceptions=len(cases), safe=cells - failed, fail=failed, conformance_credit=0)
        (destination / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        print(f'Safety exceptions: {len(cases)}; cells: safe={cells-failed}, fail={failed}; exact conformance credit=0')
        print(f'Report: {destination / "report.json"}')
        return 1 if failed else 0
    except (OSError, ValueError, KeyError, harness.HarnessError) as error:
        parser.exit(2, f'safety gate: {error}\n')


if __name__ == '__main__':
    raise SystemExit(main())
