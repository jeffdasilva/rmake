#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Controlled tests for makefile remake overlap, restart, and failure drain."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import sys
import tempfile

import debug_output as debug
from debug_output import h, reference_oracle
from job_debug import proc_identity


SOURCES = debug.ROOT / 'tests/remake-parallel'
ANCHOR = b'anchor\n'
BEFORE_FIRST = ANCHOR + b'read-\n'
BEFORE_SECOND = BEFORE_FIRST + b'left\n'
INPUT_TIME = 1_000_000_000_000_000_000
WORKERS = ('left', 'right')
WAIT_CHANNELS = {'core_sys_select', 'do_wait', '__x64_sys_poll'}
X86_64 = os.uname().machine == 'x86_64'

MISSING = b'Makefile:2: generated.gen: No such file or directory\n'
LEFT_ERROR = b'make: *** [Makefile:7: left] Error 3\n'
RIGHT_ERROR = b'make: *** [Makefile:8: right] Error 4\n'
WAITING = b'make: *** Waiting for unfinished jobs....\n'
REMAKE_ERROR = b"Makefile:2: Failed to remake makefile 'generated.gen'.\n"


@dataclass(frozen=True)
class HeldModel:
    model: debug.Model
    overlap: bool
    before_second: bytes | None
    before_first: bytes = BEFORE_FIRST


def models() -> list[HeldModel]:
    worker = (SOURCES / 'worker.py').read_bytes()
    markers = {f'{name}.{suffix}': content for name in WORKERS
               for suffix, content in (('started', b'started\n'), ('release', b'yes\n'),
                                       ('done', b'done\n'))}
    results = []
    for transport in ('pipe', 'fifo'):
        for mode in ('required-success', 'optional-success', 'serial-success',
                     'global-serial-success', 'optional-sibling-failure', 'optional-keep',
                     'required-sibling-failure', 'required-keep'):
            optional = mode.startswith('optional')
            success = mode.endswith('success')
            keep = mode.endswith('keep')
            serial = mode in ('serial-success', 'global-serial-success')
            directive = '-include' if optional else 'include'
            source = '.NOTPARALLEL:\n' if mode == 'global-serial-success' else ''
            source += '$(file >>seen,read-$(MAKE_RESTARTS))\n'
            updates = markers.copy()
            before_second = BEFORE_SECOND
            if success:
                source += (
                    f'{directive} left.gen right.gen\n'
                    '.PHONY: all\n'
                    'all: ; @$(file >>seen,goal-$(LEFT)-$(RIGHT)):\n'
                    'left.gen: ; @python3 worker.py left include\n'
                    'right.gen: ; @python3 worker.py right include\n'
                )
                updates.update({'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'})
                seen = BEFORE_SECOND + b'right\nread-1\ngoal-loaded-loaded\n'
                stdout = stderr = b''
                status = 0
            else:
                source += (
                    f'{directive} generated.gen\n'
                    '.PHONY: all\n'
                    'all: ; @$(file >>seen,goal):\n'
                    'generated.gen: joined ; @$(error failed dependencies must prevent this recipe)\n'
                    'joined: left right ; @$(error failed dependencies must prevent this recipe)\n'
                    'left: ; @python3 worker.py left fail\n'
                    'right: ; @python3 worker.py right fail\n'
                )
                stdout = b'left\nright\n'
                if optional:
                    status, stderr = 0, b''
                    if keep:
                        seen = BEFORE_SECOND + b'right\ngoal\n'
                    else:
                        before_second += b'goal\n'
                        seen = before_second + b'right\n'
                else:
                    status = 2
                    stderr = MISSING + LEFT_ERROR
                    stderr += RIGHT_ERROR + REMAKE_ERROR if keep else WAITING + RIGHT_ERROR
                    seen = BEFORE_SECOND + b'right\n'
            files = {'Makefile': source.encode(), 'worker.py': worker, 'seen': ANCHOR}
            jobs = '-j1' if mode == 'serial-success' else '-j2'
            args = ('-rR', '--no-print-directory', jobs, f'--jobserver-style={transport}')
            args += ('-k', 'all') if keep else ('all',)
            expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                        'stderr_template_hex': stderr.hex()}
            model = debug.Model(f'{mode}-{transport}', files, args, {'GNUMAKEFLAGS': ''}, expected,
                                seen, updates=updates, mtimes_ns=dict.fromkeys(files, INPUT_TIME))
            results.append(HeldModel(model, not serial, before_second))
    return (results + mixed_models(worker, markers) + immediate_launch_models(worker, markers)
            + shared_failure_models(worker, markers))


def mixed_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    left_error = b'make: *** [Makefile:6: left.gen] Error 3\n'
    right_error = b'make: *** [Makefile:7: right.gen] Error 4\n'
    right_open_error = b'Makefile:3: right.gen: No such file or directory\n'
    left_open_error = b'Makefile:3: left.gen: No such file or directory\n'
    left_remake_error = b"Makefile:2: Failed to remake makefile 'left.gen'.\n"
    cases = (
        ('optional-first-failure', '-include left.gen\ninclude right.gen\n',
         right_open_error + right_error),
        ('required-first-failure', 'include left.gen\n-include right.gen\n',
         left_error + WAITING),
        ('optional-first-restart', '-include left.gen\ninclude right.gen\n', b''),
        ('required-pair', 'include left.gen\ninclude right.gen\n',
         right_open_error + left_error + WAITING + right_error),
        ('required-pair-reversed', 'include right.gen\ninclude left.gen\n',
         left_open_error + left_error + WAITING + right_error),
        ('optional-declaration-first', '-include right.gen\ninclude left.gen\n',
         left_open_error + left_error + WAITING),
        ('required-optional-keep', 'include left.gen\n-include right.gen\n',
         left_error + left_remake_error),
    )
    results = []
    for transport in ('pipe', 'fifo'):
        for name, directives, stderr in cases:
            restart = name.endswith('restart')
            source = (
                '$(file >>seen,read-$(MAKE_RESTARTS))\n'
                + directives +
                '.PHONY: all\n'
                'all: ; @$(file >>seen,goal-$(RIGHT)):\n'
            )
            if restart:
                source += 'ifeq ($(MAKE_RESTARTS),)\n'
            source += 'left.gen: ; @python3 worker.py left fail\n'
            updates = markers.copy()
            if restart:
                source += 'right.gen: ; @python3 worker.py right include\nendif\n'
                updates['right.gen'] = b'RIGHT := loaded\n'
                seen = BEFORE_SECOND + b'right\nread-1\ngoal-loaded\n'
                stdout, status = b'left\n', 0
            else:
                source += 'right.gen: ; @python3 worker.py right fail\n'
                seen = BEFORE_SECOND + b'right\n'
                stdout, status = b'left\nright\n', 2
            files = {'Makefile': source.encode(), 'worker.py': worker, 'seen': ANCHOR}
            args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}')
            args += ('-k', 'all') if name.endswith('keep') else ('all',)
            expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                        'stderr_template_hex': stderr.hex()}
            model = debug.Model(f'mixed-{name}-{transport}', files, args, {'GNUMAKEFLAGS': ''},
                                expected, seen, updates=updates, mtimes_ns=dict.fromkeys(files, INPUT_TIME))
            results.append(HeldModel(model, True, BEFORE_SECOND))
    return results


def immediate_launch_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = ('$(file >>seen,read-$(MAKE_RESTARTS))\n'
              '.PHONY: all\n'
              'all: ; @$(file >>seen,goal):\n')
    spawn_error = b'make: ./missing-program: No such file or directory\n'
    failure = 'missing: ; @./missing-program\n'
    cases = (
        ('ignored-required', (
            '$(file >>seen,read-$(MAKE_RESTARTS))\n'
            'include left.gen\ninclude right.gen\n'
            '.PHONY: all\nall: ; @$(file >>seen,goal):\n'
            'ifeq ($(MAKE_RESTARTS),)\n'
            'left.gen: ; @python3 worker.py left include\n'
            'right.gen:\n\t-@./missing-program\n'
            '\t@printf "RIGHT := loaded\\n" > "$@"\nendif\n'
        ), 0, b'', spawn_error
            + b'Makefile:3: right.gen: No such file or directory\n'
            + b'make: [Makefile:9: right.gen] Error 127 (ignored)\n',
         BEFORE_FIRST, BEFORE_SECOND + b'read-1\ngoal\n',
         {'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}),
        ('optional-continues', header + (
            '-include generated.gen\n'
            'generated.gen: left missing third ; @$(error failed dependencies must prevent this recipe)\n'
            'left: ; @python3 worker.py left fail\n'
            'third: ; @$(file >>seen,third):\n'
        ) + failure, 0, b'left\n', spawn_error, BEFORE_FIRST + b'third\n',
         BEFORE_FIRST + b'third\nleft\ngoal\n', {}),
        ('optional-no-rule', header + (
            '-include generated.gen\n'
            'generated.gen: left missing third ; @$(error failed dependencies must prevent this recipe)\n'
            'left: ; @python3 worker.py left success\n'
            'third: ; @$(file >>seen,third):\n'
        ), 0, b'', b'', BEFORE_FIRST, BEFORE_SECOND + b'goal\n', {}),
        ('optional-wait', header + (
            '-include generated.gen\n'
            'generated.gen: left missing .WAIT third ; @$(error failed dependencies must prevent this recipe)\n'
            'left: ; @python3 worker.py left success\n'
            'third: ; @$(file >>seen,third):\n'
        ) + failure, 0, b'', spawn_error, BEFORE_FIRST, BEFORE_SECOND + b'goal\n', {}),
        ('required-drains', (
            '$(file >>seen,read-$(MAKE_RESTARTS))\n'
            '-include left.gen\ninclude right.gen\n'
            '.PHONY: all\nall: ; @$(file >>seen,goal):\n'
            'left.gen: ; @python3 worker.py left include\n'
            'right.gen: ; @./missing-program\n'
        ), 2, b'', spawn_error + b'Makefile:3: right.gen: No such file or directory\n'
            + b'make: *** [Makefile:7: right.gen] Error 127\n' + WAITING,
         BEFORE_FIRST, BEFORE_SECOND, {'left.gen': b'LEFT := loaded\n'}),
    )
    left_markers = {name: data for name, data in markers.items() if name.startswith('left.')}
    results = []
    for transport in ('pipe', 'fifo'):
        for name, source, status, stdout, stderr, before_first, seen, updates in cases:
            files = {'Makefile': source.encode(), 'worker.py': worker, 'seen': ANCHOR}
            args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}', 'all')
            expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                        'stderr_template_hex': stderr.hex()}
            model = debug.Model(f'immediate-{name}-{transport}', files, args, {'GNUMAKEFLAGS': ''},
                                expected, seen, updates={**left_markers, **updates},
                                mtimes_ns=dict.fromkeys(files, INPUT_TIME))
            results.append(HeldModel(model, False, None, before_first))
    return results


def shared_failure_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = (
        '$(file >>seen,read-$(MAKE_RESTARTS))\n'
        '-include unit.part\ninclude unit.index\n'
        '.PHONY: all\nall: ; @$(file >>seen,goal):\n'
    )
    forbidden = ' shared ; @$(error failed dependencies must prevent this recipe)\n'
    shared = 'shared: ; @python3 worker.py left fail\n'
    open_error = b'Makefile:3: unit.index: No such file or directory\n'
    needed = b"make: *** No rule to make target 'shared', needed by 'unit.index'."
    separate = 'unit.part:' + forbidden + 'unit.index:' + forbidden
    cases = (
        ('implicit', header + '%.part %.index:' + forbidden + shared, (), open_error + needed + b'  Stop.\n'),
        ('explicit', header + 'unit.part unit.index &:' + forbidden + shared, (), open_error + needed + b'  Stop.\n'),
        ('double-colon', header + 'unit.part unit.index &::' + forbidden + shared, (), open_error + needed + b'  Stop.\n'),
        ('separate', header + separate + shared, (), open_error + needed + b'  Stop.\n'),
        ('keep-going', header + separate + shared, ('-k',), open_error + needed
         + b"\nMakefile:3: Failed to remake makefile 'unit.index'.\n"),
        ('required-owner', header.replace('-include unit.part\ninclude unit.index\n',
                                          'include unit.part\n-include unit.index\n')
         + separate + shared, ('-k',), b'make: *** [Makefile:8: shared] Error 3\n'
         + b"Makefile:2: Failed to remake makefile 'unit.part'.\n"),
    )
    left_markers = {name: data for name, data in markers.items() if name.startswith('left.')}
    results = []
    for transport in ('pipe', 'fifo'):
        for name, source, flags, stderr in cases:
            files = {'Makefile': source.encode(), 'worker.py': worker, 'seen': ANCHOR}
            args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}', *flags, 'all')
            expected = {'exit_code': 2, 'stdout_template_hex': b'left\n'.hex(),
                        'stderr_template_hex': stderr.hex()}
            model = debug.Model(f'shared-failure-{name}-{transport}', files, args, {'GNUMAKEFLAGS': ''},
                                expected, BEFORE_SECOND, updates=left_markers,
                                mtimes_ns=dict.fromkeys(files, INPUT_TIME))
            results.append(HeldModel(model, False, None))
    return results


def active_workers(parent: int, proc: Path) -> dict[str, tuple[int, int]]:
    workers = {}
    for token in (proc / 'task' / str(parent) / 'children').read_bytes().split():
        pid = int(token)
        try:
            command = Path('/proc', str(pid), 'cmdline').read_bytes().split(b'\0')
            if len(command) != 5 or command[:2] != [b'python3', b'worker.py']:
                continue
            name = command[2].decode('ascii')
            owner, started_at = proc_identity(pid)
        except (FileNotFoundError, ProcessLookupError):
            continue
        if name not in WORKERS or name in workers or owner != parent or started_at <= 0:
            raise h.HarnessError('invalid held remake worker identity')
        workers[name] = (pid, started_at)
    return workers


def poll_wait(proc: Path) -> tuple[int | None, bool]:
    if not X86_64:
        return None, False
    try:
        fields = (proc / 'syscall').read_text().split()
        if len(fields) != 9:
            return None, False
        number = int(fields[0])
        argument = int(fields[3], 0)
        # Linux x86-64 poll takes a signed millisecond timeout. ppoll takes
        # a pointer to a timespec, with NULL denoting an indefinite wait.
        blocking = ((number == 7 and argument & 0xffffffff != 0)
                    or (number == 271 and argument == 0))
        return number, blocking
    except (OSError, ValueError):
        return None, False


def execute(case: h.Case, runtime: Path, binary: Path,
            wait_samples: set[tuple[str, int | None]] | None = None):
    work = runtime / 'work'
    releases = []
    released = set()
    previous = None

    def observe(parent: int, stdout: bytes) -> None:
        nonlocal previous
        if len(released) == len(WORKERS):
            return
        proc = Path('/proc', str(parent))
        try:
            channel = (proc / 'wchan').read_text().strip()
            number, blocked = (None, True) if channel in WAIT_CHANNELS else poll_wait(proc)
            if wait_samples is not None:
                wait_samples.add((channel, number))
            if not blocked:
                return
            workers = active_workers(parent, proc)
            if not workers or released.intersection(workers):
                return
            for name in workers:
                if (work / f'{name}.started').read_bytes() != b'started\n':
                    raise h.HarnessError('unexpected held remake worker marker')
                if (work / f'{name}.done').exists() or (work / f'{name}.release').exists():
                    raise h.HarnessError('remake worker escaped its release barrier')
            seen = (work / 'seen').read_bytes()
        except (FileNotFoundError, ProcessLookupError):
            return
        # Let a serial baseline progress when it demonstrably waits on only one
        # child. The oracle and candidate still have to match the expected set.
        signature = (sorted(workers.items()), channel, number, seen, stdout)
        if signature != previous:
            previous = signature
            return
        name = min(workers)
        temporary = work / f'{name}.release.tmp'
        temporary.write_bytes(b'yes\n')
        temporary.chmod(0o644)
        temporary.rename(work / f'{name}.release')
        releases.append({'name': name, 'parent': parent, 'workers': workers,
                         'wchan': channel, 'poll_syscall': number,
                         'seen_hex': seen.hex(), 'stdout_hex': stdout.hex()})
        released.add(name)

    return h.execute(case, runtime, binary, observe=observe), releases


def release_errors(held: HeldModel, releases: list[dict]) -> list[str]:
    order = ['left'] if held.before_second is None else list(WORKERS)
    if [release['name'] for release in releases] != order:
        return ['held release sequence']
    errors = []
    first = releases[0]
    expected_first = list(WORKERS) if held.overlap else ['left']
    if sorted(first['workers']) != expected_first:
        errors.append('active workers at first release')
    if first['seen_hex'] != held.before_first.hex():
        errors.append('file effects at first release')
    if first['stdout_hex']:
        errors.append('stdout at first release')
    if held.before_second is None:
        return errors
    second = releases[1]
    if list(second['workers']) != ['right']:
        errors.append('active workers at second release')
    if first['parent'] != second['parent']:
        errors.append('parent identity at release')
    if held.overlap and 'right' in first['workers'] and first['workers']['right'] != second['workers'].get('right'):
        errors.append('right identity across releases')
    if second['seen_hex'] != held.before_second.hex():
        errors.append('file effects at second release')
    stdout_after_left = b'left\n' if held.model.expected['stdout_template_hex'] else b''
    if second['stdout_hex'] != stdout_after_left.hex():
        errors.append('stdout at second release')
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=reference_oracle.PREFIX / 'bin' / 'make')
    parser.add_argument('--oracle-profile', type=Path, default=reference_oracle.PROFILE)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--self-test', action='store_true')
    mode.add_argument('--candidate', type=Path)
    parser.add_argument('--case', action='append', default=[])
    reports = parser.add_mutually_exclusive_group()
    reports.add_argument('--report', type=Path)
    reports.add_argument('--report-dir', type=Path)
    args = parser.parse_args()
    try:
        profile = reference_oracle.verify(args.oracle, args.oracle_profile)
        available = models()
        unknown = set(args.case) - {held.model.id for held in available}
        if unknown:
            raise h.HarnessError('unknown remake cases: ' + ', '.join(sorted(unknown)))
        selected = [held for held in available if not args.case or held.model.id in args.case]
        executables = [args.oracle, args.oracle]
        if args.candidate is not None:
            executables.append(args.candidate)
        records = []
        with tempfile.TemporaryDirectory(prefix='rmake-remake-parallel-') as scratch:
            root = Path(scratch)
            runtime = root / 'runtime'
            runtime.mkdir(mode=0o755)
            for held in selected:
                model = held.model
                case, filesystem = debug.prepare(model, root / model.id, runtime)
                samples = [set() for _ in executables]
                observations, releases = zip(*(
                    execute(case, runtime, binary, waits)
                    for binary, waits in zip(executables, samples, strict=True)
                ))
                record = debug.assess(model, case, runtime, filesystem, list(observations))
                errors = [release_errors(held, release) for release in releases]
                wait_samples = [sorted(waits, key=lambda pair: (pair[0], pair[1] or -1))
                                for waits in samples]
                record.update(id=model.id, releases=releases, release_errors=errors,
                              wait_samples=wait_samples,
                              passed=record['passed'] and not any(errors))
                records.append(record)
                if record['passed']:
                    print('PASS', model.id, flush=True)
                else:
                    print('FAIL', model.id,
                          {key: record[key] for key in (
                              'expected_errors', 'filesystem_errors', 'oracle_repeat_differences',
                              'candidate_differences', 'release_errors', 'wait_samples')}, flush=True)
        total = len(records)
        passed = sum(record['passed'] for record in records)
        report = {'oracle_profile': profile['profile_id'],
                  'executables': [h.executable_identity(binary) for binary in executables],
                  'totals': {'cases': total, 'pass': passed, 'fail': total - passed},
                  'cases': records}
        report_path = args.report
        if args.report_dir is not None:
            args.report_dir.mkdir(parents=True, exist_ok=True)
            report_path = Path(tempfile.mkdtemp(prefix='run-', dir=args.report_dir)) / 'report.json'
        if report_path is not None:
            with report_path.open('x', encoding='utf-8') as output:
                json.dump(report, output, indent=2)
                output.write('\n')
            print(f'Report: {report_path}')
        print(f'Parallel makefile remakes: {passed}/{total} passed')
        return int(passed != total)
    except (h.HarnessError, reference_oracle.OracleError, OSError) as error:
        print(f'remake-parallel: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
