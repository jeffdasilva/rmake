#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original JOBS protocol models with independently witnessed live identities."""
from __future__ import annotations
import argparse
import base64
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile

import conformance as h
import oracle

ROOT = Path(__file__).resolve().parents[1]
WORKER = ROOT / 'tests/job-debug/worker.py'


@dataclass(frozen=True)
class Model:
    id: str
    source: bytes
    workers: tuple[str, ...]
    events: tuple[tuple[str, str, str], ...]
    seen: bytes
    status: int = 0
    stderr: bytes = b''
    style: str | None = None
    borrow: bool = False


def models():
    def cycle(job, worker, losing=False):
        return [('put', job, worker), ('live', job, worker),
                ('lose' if losing else 'win', job, worker), ('remove', job, worker)]
    one = b'.PHONY: all\nall:\n\t@python3 worker.py one serial 0\n'
    serial = b'anchor\nSTART-one\nEND-one\n'
    result = [Model('jobs-serial-one', one, ('one',), tuple(cycle('all', 'one')), serial)]
    multi = one + b'\t@python3 worker.py two serial 0\n'
    events = cycle('all', 'one')[:3] + cycle('all', 'two')[1:]
    twice = serial + b'START-two\nEND-two\n'
    result.append(Model('jobs-serial-multiline', multi, ('one', 'two'), tuple(events), twice))
    targets = b'.PHONY: all one two\nall: one two\none:\n\t@python3 worker.py one serial 0\ntwo:\n\t@python3 worker.py two serial 0\n'
    result.append(Model('jobs-serial-targets', targets, ('one', 'two'),
                        tuple(cycle('one', 'one') + cycle('two', 'two')), twice))
    fail = one.replace(b'serial 0', b'serial 3')
    result.append(Model('jobs-failure', fail, ('one',), tuple(cycle('all', 'one', True)),
                        serial, 2, b'make: *** [Makefile:3: all] Error 3\n'))
    result.append(Model('jobs-ignored-failure', fail.replace(b'@python', b'-@python'),
                        ('one',), tuple(cycle('all', 'one', True)), serial, 0,
                        b'make: [Makefile:3: all] Error 3 (ignored)\n'))
    source = b'.PHONY: all first second\nall: first second\nfirst:\n\t+@python3 worker.py first pair 0\nsecond:\n\t+@python3 worker.py second pair 0\n'
    sequence = [('controller', '', ''), ('need-empty', '', ''), ('put', 'first', 'first'),
                ('live', 'first', 'first'), ('need-live', '', ''), ('live', 'first', 'first'),
                ('obtained', 'second', 'second'), ('put', 'second', 'second'),
                ('live', 'second', 'second'), ('live', 'first', 'first'),
                ('win', 'first', 'first'), ('remove', 'first', 'first'),
                ('released', 'first', 'first'), ('live', 'second', 'second'),
                ('live', 'second', 'second'), ('win', 'second', 'second'),
                ('remove', 'second', 'second'), ('drain', '', '')]
    for style in ('fifo', 'pipe'):
        result.append(Model('jobs-parallel-' + style, source, ('first', 'second'), tuple(sequence),
                            b'anchor\nSTART-first\nSTART-second\nEND-first\nEND-second\n', style=style))
        result.append(Model('jobs-returned-token-' + style, source.replace(b' pair ', b' loan '),
                            ('first', 'second'), tuple(sequence),
                            b'anchor\nSTART-first\nSTART-second\nEND-first\nEND-second\n',
                            style=style, borrow=True))
    return result


def file_entry(content):
    return {'type': 'file', 'mode': 0o644, 'size': len(content),
            'sha256': hashlib.sha256(content).hexdigest(),
            'content': base64.b64encode(content).decode()}


def proc_identity(pid):
    fields = Path('/proc', str(pid), 'stat').read_bytes().rsplit(b') ', 1)[1].split()
    return int(fields[1]), int(fields[19])


WIRE = re.compile(rb'pid=([1-9][0-9]*) ppid=([1-9][0-9]*) start=([1-9][0-9]*) target=([a-z]+)\n\Z')


def witness(raw, name, make_pid):
    match = WIRE.fullmatch(raw)
    if match is None or match[4] != name.encode():
        raise h.HarnessError('invalid worker identity wire')
    pid, parent, start = map(int, match.group(1, 2, 3))
    if parent != make_pid or proc_identity(pid) != (parent, start):
        raise h.HarnessError('worker identity disagrees with live process')
    cmdline = Path('/proc', str(pid), 'cmdline').read_bytes().split(b'\0')
    if cmdline[:3] != [b'python3', b'worker.py', name.encode()]:
        raise h.HarnessError('PID is not the declared recipe worker')
    return {'pid': pid, 'parent': parent, 'start': start}


def execute(model, case, runtime, binary):
    h.reset_workspace(case, runtime, binary)
    host = {'make_pid': None, 'workers': {}}
    def observe(make_pid, stdout):
        if host['make_pid'] not in (None, make_pid):
            raise h.HarnessError('make process identity changed')
        host['make_pid'] = make_pid
        for name in model.workers:
            path = runtime / 'work' / (name + '.identity')
            if name not in host['workers'] and path.exists():
                host['workers'][name] = witness(path.read_bytes(), name, make_pid)
                acknowledgment = runtime / 'work' / (name + '.verified')
                acknowledgment.write_bytes(b'yes\n')
                acknowledgment.chmod(0o644)
        if model.style and 'second' in host['workers']:
            pid = str(host['workers']['second']['pid']).encode()
            pattern = rb'(?m)^Live child 0x[0-9a-f]+ \(second\) PID ' + pid + rb' \n'
            if len(re.findall(pattern, stdout)) >= 2:
                finish = runtime / 'work/finish-second'
                if not finish.exists():
                    finish.write_bytes(b'yes\n')
                    finish.chmod(0o644)
    out, err, status, limit, leftovers = h.capture(
        [str(runtime / 'bin/make'), *case.args], runtime / 'work',
        h.clean_environment(runtime, case.environment), case.timeout, observe=observe)
    files = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    return h.Observation(out, err, status, limit, files, leftovers), host


PREFIX = {'put': b'Putting child ', 'live': b'Live child ', 'win': b'Reaping winning child ',
          'lose': b'Reaping losing child ', 'remove': b'Removing child ',
          'obtained': b'Obtained token for child ', 'released': b'Released token for child '}


def protocol_errors(model, stdout, host, runtime):
    lines = stdout.splitlines(keepends=True)
    if len(lines) != len(model.events):
        return ['job event count']
    bindings, active = {}, {}
    errors = []
    for index, (line, (op, job, worker)) in enumerate(zip(lines, model.events)):
        pointer = None
        if op in PREFIX:
            match = re.match(re.escape(PREFIX[op]) + rb'(0x[0-9a-f]+)', line)
            if match is None:
                errors.append(f'event {index}: syntax'); continue
            pointer = match[1]
            if int(pointer, 16) < 4096 or int(pointer, 16) % 8:
                errors.append(f'event {index}: invalid pointer')
            if job not in bindings:
                if op not in ('put', 'obtained') or pointer in active:
                    errors.append(f'event {index}: new identity')
                bindings[job] = pointer
            if bindings[job] != pointer:
                errors.append(f'event {index}: unstable identity')
            if op in ('put', 'obtained'):
                if pointer in active and active[pointer] != job:
                    errors.append(f'event {index}: concurrent identity reuse')
                active[pointer] = job
            elif op != 'released' and active.get(pointer) != job:
                errors.append(f'event {index}: inactive identity')
        data = host['workers'].get(worker, {})
        pid = str(data.get('pid', -1)).encode()
        target = job.encode()
        if op == 'put':
            expected = PREFIX[op] + pointer + b' (' + target + b') PID ' + pid + b' on the chain.\n'
        elif op == 'live':
            expected = PREFIX[op] + pointer + b' (' + target + b') PID ' + pid + b' \n'
        elif op in ('win', 'lose'):
            expected = PREFIX[op] + pointer + b' PID ' + pid + b' \n'
        elif op == 'remove':
            expected = PREFIX[op] + pointer + b' PID ' + pid + b' from chain.\n'
            active.pop(pointer, None)
        elif op in ('obtained', 'released'):
            expected = PREFIX[op] + pointer + b' (' + target + b').\n'
        elif op == 'controller':
            value = (b'fifo:' + os.fsencode(runtime / 'tmp') + b'/GMfifo' + str(host['make_pid']).encode()
                     if model.style == 'fifo' else b'3,4')
            expected = b'Using jobserver controller ' + value + b'\n'
        elif op == 'need-empty': expected = b"Need a job token; we don't have children\n"
        elif op == 'need-live': expected = b'Need a job token; we have children\n'
        elif op == 'drain': expected = b'Acquired all 1 jobserver tokens.\n'
        else: raise h.HarnessError('unknown model operation')
        if line != expected:
            errors.append(f'event {index}: exact bytes/PID')
    if active:
        errors.append('unremoved jobs')
    return errors


def assess(model, observation, host, runtime, initial):
    errors = []
    if (observation.returncode != model.status or observation.stderr != model.stderr
            or observation.limit or observation.leftover_processes):
        errors.append('status/stderr/process bounds')
    if set(host['workers']) != set(model.workers) or not host['make_pid']:
        errors.append('missing live identity witnesses')
    identities = []
    for data in host['workers'].values():
        if (set(data) != {'pid', 'parent', 'start'}
                or any(type(value) is not int or value <= 0 for value in data.values())
                or data.get('parent') != host['make_pid'] or data.get('pid') == host['make_pid']):
            errors.append('invalid host identity evidence')
        identities.append((data.get('pid'), data.get('start')))
    if len(set(identities)) != len(identities):
        errors.append('worker process identity reused')
    expected = dict(initial)
    expected['work/seen'] = file_entry(model.seen)
    for name in model.workers:
        data = host['workers'].get(name, {})
        content = f"pid={data.get('pid')} ppid={data.get('parent')} start={data.get('start')} target={name}\n".encode()
        expected['work/' + name + '.identity'] = file_entry(content)
        for suffix in ('verified', 'started', 'done'):
            expected['work/' + name + '.' + suffix] = file_entry(b'yes\n')
    if model.style:
        expected['work/finish-second'] = file_entry(b'yes\n')
    if model.borrow:
        expected['work/borrowed-token'] = file_entry(b'borrowed=+ returned=+\n')
    if observation.filesystem != expected:
        errors.append('independent complete filesystem')
    errors.extend(protocol_errors(model, observation.stdout, host, runtime))
    return errors


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case', action='append')
    args = parser.parse_args(argv)
    selected = models()
    if args.case:
        if set(args.case) - {m.id for m in selected}: parser.error('unknown exact case')
        selected = [m for m in selected if m.id in args.case]
    oracle.verify(args.oracle.resolve())
    binaries = [args.oracle.resolve(), args.oracle.resolve()]
    if args.candidate: binaries.append(args.candidate.resolve())
    report = {'classification': 'Original JOBS qualification; zero ordinary/safety credit',
              'tool_sha256': h.sha256_file(Path(__file__)), 'worker_sha256': h.sha256_file(WORKER),
              'observer_sha256': h.sha256_file(Path(h.__file__)),
              'identities': [h.executable_identity(b) for b in binaries], 'cases': []}
    with tempfile.TemporaryDirectory(prefix='rmake-job-debug-') as temporary:
        root = Path(temporary)
        runtime = root / 'runtime'
        runtime.mkdir(mode=0o755)
        # This path is chosen before execution, never extracted from output.
        report['runtime_root'] = str(runtime)
        for model in selected:
            fixture = root / model.id
            fixture.mkdir(mode=0o755)
            for name, content in {'Makefile': model.source, 'worker.py': WORKER.read_bytes(), 'seen': b'anchor\n'}.items():
                (fixture / name).write_bytes(content)
            initial = h.snapshot({'work': fixture})
            initial.update({name: {'type': 'directory', 'mode': 0o755} for name in ('home', 'tmp')})
            arguments = ('-rR', '--no-print-directory', '--debug=j')
            if model.style: arguments += ('-j2', '--jobserver-style=' + model.style)
            case = h.Case(fixture, model.id, 'debug.jobs', 'required', arguments, 6, {'GNUMAKEFLAGS': ''}, {})
            row = {'id': model.id, 'arguments': arguments, 'expected_events': model.events,
                   'makefile_hex': model.source.hex(), 'expected_seen_hex': model.seen.hex(),
                   'initial_filesystem': initial, 'observations': [], 'errors': []}
            for binary in binaries:
                observation, host = execute(model, case, runtime, binary)
                errors = assess(model, observation, host, runtime, initial)
                row['errors'].append(errors)
                row['observations'].append({**h.observation_json(observation), 'stdout_hex': observation.stdout.hex(),
                                            'stderr_hex': observation.stderr.hex(), 'host': host})
            row['passed'] = not any(row['errors'])
            report['cases'].append(row)
            print(('PASS ' if row['passed'] else 'FAIL ') + model.id, row['errors'], flush=True)
    report['passed'] = all(row['passed'] for row in report['cases'])
    report['totals'] = {'cases': len(report['cases']),
                        'pass': sum(row['passed'] for row in report['cases']),
                        'fail': sum(not row['passed'] for row in report['cases'])}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
