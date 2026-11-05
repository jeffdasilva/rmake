#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare exact detached-worker outcomes with a causal process wait witness."""
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
import conformance as h
import guile_frontend as g

PLAN = ROOT / 'tests/guile-detached/models.json'
PLAN_SHA256 = '8219b6bb6b98bd6e9ae0b61326b3542eb2e10b6d3375e2b3b5e70572b80e1e4b'


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--case', action='append')
    parser.add_argument('--report', type=Path, required=True)
    return parser.parse_args()


args = arguments()
if args.report.exists():
    raise h.HarnessError('refusing to replace retained results')
if os.uname().machine != 'x86_64' or not sys.platform.startswith('linux'):
    raise h.HarnessError('detached wait observer requires Linux x86-64 syscall identity')
if h.sha256_file(PLAN) != PLAN_SHA256:
    raise h.HarnessError('detached worker model digest changed')
profile = g.read_profile(args.profile)
model = json.loads(PLAN.read_bytes())
reference = Path(profile['binary']['path'])
candidate = args.candidate.resolve() if args.candidate else None
engines = [reference, reference, *([candidate] if candidate else [])]
release_syscalls = model['host_release']
environment = {key: profile['environment'][key] for key in g.ENVIRONMENT}
environment['GNUMAKEFLAGS'] = ''
selected = [row for row in model['cases'] if not args.case or row['id'] in args.case]
if len(model['cases']) != 6 or not selected or (args.case and {row['id'] for row in selected} != set(args.case)):
    raise h.HarnessError('unknown detached worker model')
args.report.parent.mkdir(parents=True, exist_ok=True)
report = {'classification': model['classification'], 'plan_sha256': PLAN_SHA256,
          'observer_sha256': h.sha256_file(Path(__file__)),
          'profile_sha256': h.sha256_file(args.profile),
          'candidate': h.executable_identity(candidate) if candidate else None,
          'gnu': profile['binary'], 'affinity': sorted(os.sched_getaffinity(0)),
          'rows': [], 'passed': False}


def checkpoint():
    staging = args.report.with_suffix('.next.json')
    staging.write_text(json.dumps(report, indent=2) + '\n')
    staging.replace(args.report)


def blocking_wait(raw, accepted):
    fields = raw.split()
    if len(fields) != 9 or not fields[0].isdigit():
        return None
    name = accepted.get(int(fields[0]))
    if name is None:
        return None
    try:
        third = int(fields[3], 0)
        fourth = int(fields[4], 0)
    except ValueError:
        return None
    if name == 'poll':
        return name if third & 0xffffffff else None
    if name in ('rt_sigtimedwait', 'ppoll'):
        return name if third == 0 else None
    options = third if name == 'wait4' else fourth
    return name if not (options & 1) else None


def callback_release(work, accepted_syscalls, row):
    gates = row['gates']
    witness = {'polls_at_ready': 0, 'raw_last_syscall': None, 'releases': []}
    inverse = {number: name for name, number in accepted_syscalls.items()}

    def release_file(name):
        temporary = work / (name + '.tmp')
        temporary.write_bytes(b'go\n')
        temporary.chmod(0o644)
        temporary.rename(work / name)

    def observe(pid, _stdout):
        if len(witness['releases']) >= len(gates):
            return
        gate = gates[len(witness['releases'])]
        ready = work / gate['ready']
        if not ready.is_file() or ready.read_bytes() != b'waiting\n':
            return
        witness['polls_at_ready'] += 1
        try:
            syscall = (Path('/proc') / str(pid) / 'syscall').read_text().strip()
        except OSError as err:
            witness['last_proc_errno'] = err.errno
            return
        witness['raw_last_syscall'] = syscall
        name = blocking_wait(syscall, inverse)
        if name is not None:
            witness['releases'].append({'observed_leading_pid': pid, 'syscall': name,
                                        'raw_proc_syscall': syscall, 'ready_hex': ready.read_bytes().hex(),
                                        'gate': gate})
            release_file(gate['release'])

    return observe, witness


with tempfile.TemporaryDirectory(prefix='rmake-guile-detached-') as tmp:
    private = Path(tmp)
    runtime = private / 'runtime'
    runtime.mkdir()
    work = runtime / 'work'
    roots = {name: runtime / name for name in ('work', 'home', 'tmp')}
    for row in selected:
        result = {'case': row['id'], 'runs': []}
        report['rows'].append(result)
        source = private / g.safe_relative(row['id'])
        source.mkdir()
        for name, text in row['files'].items():
            (source / g.safe_relative(name)).write_bytes(text.encode())
        case = h.Case(source, row['id'], 'guile.detached', 'required', tuple(row['argv']),
                      4, environment, {})
        expected = row['expected']
        for engine in engines:
            h.reset_workspace(case, runtime, engine)
            filesystem = h.snapshot(roots)
            for name, content in expected['changed'].items():
                filesystem['work/' + name].update(g.file_fact(content.encode()))
            for name, content in expected['added'].items():
                filesystem['work/' + name] = {'mode': 0o644, 'type': 'file', **g.file_fact(content.encode())}
            result['expected_filesystem'] = filesystem
            observe = None
            witness = None
            if row['gate'] == 'recipe':
                observe, witness = callback_release(
                    work, release_syscalls['gnu_x86_64_syscalls' if engine == reference else 'rmake_x86_64_syscalls'], row)
            stdout, stderr, status, limit, leftovers = h.capture(
                [str(runtime / 'bin/make'), *case.args], work, h.clean_environment(runtime, environment),
                case.timeout, observe=observe)
            seen = h.Observation(stdout, stderr, status, limit, h.snapshot(roots), leftovers)
            wanted = h.Observation(expected['stdout'].encode(), expected['stderr'].encode(),
                                   expected['returncode'], expected['limit'], filesystem, expected['leftover_processes'])
            diffs = h.differences(wanted, seen)
            if witness is not None and len(witness['releases']) != len(row['gates']):
                diffs.append('causal child-wait witness missing')
            result['runs'].append({'engine': str(engine), **h.observation_json(seen), 'stdout_hex': stdout.hex(),
                                   'stderr_hex': stderr.hex(), 'observer': witness, 'differences': diffs})
            checkpoint()
            if limit or leftovers or status < 0:
                print(row['id'], 'UNQUALIFIED', diffs, flush=True)
                raise SystemExit(1)
        result['passed'] = all(not run['differences'] for run in result['runs'])
        print(row['id'], 'PASS' if result['passed'] else 'FAIL',
              [run['differences'] for run in result['runs']], flush=True)
        checkpoint()
report['passed'] = all(row['passed'] for row in report['rows'])
checkpoint()
raise SystemExit(0 if report['passed'] else 1)
