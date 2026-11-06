#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact parallel models with two proven live children and an owned wait."""
import argparse
import json
import os
import re
from pathlib import Path
import stat
import sys
import tempfile

import conformance as h
import guile_frontend as g

ROOT = Path(__file__).resolve().parents[1]
PLAN = ROOT / 'tests/guile-parallel-detached/models.json'
PLAN_SHA256 = '6bd22432ef2b486fdb0ff4907e0cc1bf0b8359585a11af1979ac3177eb020cdd'
PYTHON = Path('/usr/bin/python3').resolve()


def fd_flags(fdinfo):
    line = next(line for line in fdinfo.read_text().splitlines() if line.startswith('flags:'))
    return int(line.split()[1], 8)


def transport(proc, runtime, style, auth, pid):
    if style == 'pipe':
        matched = re.fullmatch(rb'--jobserver-auth=([1-9][0-9]*),([1-9][0-9]*)\n', auth)
        if matched is None:
            return None
        reader, writer = (int(value) for value in matched.groups())
        if min(reader, writer) < 3 or reader == writer:
            return None
        pair = [proc / 'fd' / str(fd) for fd in (reader, writer)]
        links = [os.readlink(path) for path in pair]
        if (not links[0].startswith('pipe:[') or links[0] != links[1]
                or not all(stat.S_ISFIFO(path.stat().st_mode) for path in pair)
                or fd_flags(proc / 'fdinfo' / str(reader)) & 3 != os.O_RDONLY
                or fd_flags(proc / 'fdinfo' / str(writer)) & 3 != os.O_WRONLY):
            return None
        return {'read_fds': [reader], 'write_fd': writer, 'linked_pipe': links[0]}
    path = runtime / 'tmp' / f'GMfifo{pid}'
    if auth != f'--jobserver-auth=fifo:{path}\n'.encode() or not stat.S_ISFIFO(path.stat().st_mode):
        return None
    descriptors = []
    expected = path.stat()
    for fd in (proc / 'fd').iterdir():
        try:
            linked = fd.stat()
        except FileNotFoundError:
            continue
        if linked.st_dev == expected.st_dev and linked.st_ino == expected.st_ino:
            if fd_flags(proc / 'fdinfo' / fd.name) & 3 != os.O_WRONLY:
                descriptors.append(int(fd.name))
    if descriptors:
        return {'read_fds': sorted(descriptors), 'fifo_inode': expected.st_ino, 'fifo_path': str(path)}
    return None


def live_workers(proc, pid):
    selected = {}
    expected = {'alpha', 'beta'}
    for child in (proc / 'task' / str(pid) / 'children').read_text().split():
        path = Path('/proc') / child
        try:
            arguments = (path / 'cmdline').read_bytes().split(b'\0')
            fields = (path / 'stat').read_text().rsplit(')', 1)[1].split()
            exe = (path / 'exe').resolve(strict=True)
        except OSError:
            continue
        if (len(arguments) != 4 or arguments[1] != b'worker.py' or arguments[3] != b''
                or fields[0] in ('Z', 'X') or int(fields[1]) != pid or int(fields[2]) != pid
                or exe != PYTHON):
            continue
        role = arguments[2].decode()
        if role in expected and role not in selected:
            selected[role] = {'pid': int(child), 'state': fields[0], 'start_time': fields[19],
                              'argv_hex': [argument.hex() for argument in arguments[:-1]]}
        else:
            return None
    return selected if set(selected) == expected else None


def wait_kind(raw, style, reader_fds, oracle):
    fields = raw.split()
    if len(fields) != 9 or not fields[0].isdigit():
        return None
    number = int(fields[0])
    try:
        one, two, three, four, five, _ = (int(x, 0) for x in fields[1:7])
    except ValueError:
        return None
    if number == 7 and one and two >= 2 and three & 0xffffffff:
        return 'poll'
    if not oracle:
        return None
    if number == 61 and not three & 1:
        return 'wait4'
    if number == 247 and not four & 1:
        return 'waitid'
    if number in (23, 270) and five == 0 and one > 0:
        return 'select' if number == 23 else 'pselect6'
    if number in (128, 271) and three == 0:
        return 'rt_sigtimedwait' if number == 128 else 'ppoll'
    if number == 130:
        return 'rt_sigsuspend'
    if number == 0 and one in reader_fds and three > 0:
        return style + '-token-read'
    return None


def watcher(runtime, row, oracle):
    work = runtime / 'work'
    record = {'last_observation': None, 'released': None}

    def observe(pid, _stdout):
        if record['released'] is not None:
            return
        proc = Path('/proc') / str(pid)
        try:
            auth = (work / 'transport.auth').read_bytes()
            details = {
                'pid': pid, 'auth_hex': auth.hex(),
                'wait': (proc / 'syscall').read_text().strip(),
                'alpha_ready': (work / 'alpha.ready').read_bytes().hex(),
                'beta_ready': (work / 'beta.ready').read_bytes().hex(),
                'callback_done': (work / 'callback.done').exists(),
                'gamma_queued': (work / 'gamma.queued').is_file(),
                'gamma_started': (work / 'gamma.result').exists(),
            }
            record['last_observation'] = details
            if (details['alpha_ready'] != b'alpha\n'.hex() or details['beta_ready'] != b'beta\n'.hex()
                    or details['callback_done'] or details['gamma_started']
                    or details['gamma_queued'] != (row['wait'] == 'reserve')):
                return
            jobs = live_workers(proc, pid)
            pair = transport(proc, runtime, row['style'], auth, pid)
            details.update({'live_workers': jobs, 'transport': pair})
            if not jobs or not pair:
                return
            kind = wait_kind(details['wait'], row['style'], pair['read_fds'], oracle)
            if not kind:
                return
            if oracle:
                waits_for_child = kind in ('wait4', 'waitid')
                if waits_for_child != (row['wait'] == 'drain'):
                    return
            details.update({'live_workers': jobs, 'transport': pair, 'wait_kind': kind})
            record['released'] = details
            tmp = work / 'observer.tmp'
            tmp.write_bytes(b'go\n')
            tmp.chmod(0o644)
            tmp.rename(work / 'callback.release')
        except OSError as error:
            record['last_error'] = str(error)

    return observe, record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--case', action='append')
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    if args.report.exists():
        parser.error('refusing to replace retained evidence')
    if os.uname().machine != 'x86_64' or not sys.platform.startswith('linux'):
        parser.error('parallel wait observer requires Linux x86-64')
    if h.sha256_file(PLAN) != PLAN_SHA256:
        parser.error('parallel wait model digest changed')
    profile = g.read_profile(args.profile)
    reference = Path(profile['binary']['path'])
    plan = json.loads(PLAN.read_bytes())
    selected = [row for row in plan['cases'] if not args.case or row['id'] in args.case]
    if len(plan['cases']) != 10 or not selected or (args.case and set(args.case) != {row['id'] for row in selected}):
        parser.error('unknown parallel detached worker model')
    engines = [reference, reference]
    if args.candidate:
        engines.append(args.candidate.resolve())
    args.report.parent.mkdir(parents=True, exist_ok=True)
    environment = {key: profile['environment'][key] for key in g.ENVIRONMENT}
    environment['GNUMAKEFLAGS'] = ''
    report = {'profile_sha256': h.sha256_file(args.profile), 'plan_sha256': h.sha256_file(PLAN),
              'observer_sha256': h.sha256_file(Path(__file__)), 'affinity': sorted(os.sched_getaffinity(0)),
              'engines': [h.executable_identity(engine) for engine in dict.fromkeys(engines)], 'cases': []}

    def save():
        interim = args.report.with_suffix('.next.json')
        interim.write_text(json.dumps(report, indent=2) + '\n')
        interim.replace(args.report)

    with tempfile.TemporaryDirectory(prefix='rmake-guile-parallel-observe-') as temp:
        root = Path(temp)
        runtime = root / 'runtime'
        runtime.mkdir()
        roots = {name: runtime / name for name in ('work', 'home', 'tmp')}
        work = roots['work']
        for row in selected:
            case_report = {'id': row['id'], 'runs': []}
            report['cases'].append(case_report)
            source = root / g.safe_relative(row['id'])
            source.mkdir()
            for name, text in row['files'].items():
                target = source / g.safe_relative(name)
                target.write_bytes(text.encode())
                if name == "Makefile":
                    os.utime(target, (1_600_000_000, 1_600_000_000))
            case = h.Case(source, row['id'], 'guile.detached.parallel', 'required',
                          tuple(row['args']), 4, environment, {})
            for engine in engines:
                h.reset_workspace(case, runtime, engine)
                desired = h.snapshot(roots)
                expected = row['expected']
                for name, content in expected['changed'].items():
                    desired['work/' + name].update(g.file_fact(content.encode()))
                observe, witnessed = watcher(runtime, row, engine == reference)
                stdout, stderr, status, limit, leftovers = h.capture(
                    [str(runtime / 'bin/make'), *case.args], work, h.clean_environment(runtime, environment),
                    case.timeout, observe=observe)
                created = expected['created'].copy()
                witness = witnessed['released']
                if row['style'] == 'pipe' and witness:
                    pair = witness['transport']
                    created['transport.auth'] = f"--jobserver-auth={pair['read_fds'][0]},{pair['write_fd']}\n"
                elif witness:
                    created['transport.auth'] = f"--jobserver-auth=fifo:{runtime / 'tmp' / ('GMfifo' + str(witness['pid']))}\n"
                for name, content in created.items():
                    desired['work/' + name] = {'type': 'file', 'mode': 0o644, **g.file_fact(content.encode())}
                seen = h.Observation(stdout, stderr, status, limit, h.snapshot(roots), leftovers)
                wanted = h.Observation(expected['stdout'].encode(), expected['stderr'].encode(),
                                       expected['status'], None, desired)
                differences = h.differences(wanted, seen)
                if witness is None:
                    differences.append('causal two-child wait not observed')
                case_report['runs'].append({'engine': str(engine), **h.observation_json(seen),
                                            'stdout_hex': stdout.hex(), 'stderr_hex': stderr.hex(),
                                            'differences': differences, 'expected_filesystem': desired,
                                            'witness': witnessed})
                save()
                if limit or leftovers or status < 0:
                    print(row['id'], 'UNQUALIFIED', differences, flush=True)
                    return 1
            case_report['passed'] = all(not run['differences'] for run in case_report['runs'])
            print(row['id'], 'PASS' if case_report['passed'] else 'FAIL',
                  [run['differences'] for run in case_report['runs']], flush=True)
            save()
    report['passed'] = all(row['passed'] for row in report['cases'])
    save()
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
