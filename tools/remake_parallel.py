#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Controlled tests for makefile remake overlap, restart, and failure drain."""
from __future__ import annotations

import argparse
from collections.abc import Callable
from dataclasses import dataclass, replace
import json
import os
from pathlib import Path
import sys
import tempfile
import tomllib

import debug_output as debug
from debug_output import h, reference_oracle
from job_debug import proc_identity
from job_debug_load import compile_fixture
from load_limits import SOURCE as LOAD_SHIM_SOURCE


SOURCES = debug.ROOT / 'tests/remake-parallel'
ANCHOR = b'anchor\n'
BEFORE_FIRST = ANCHOR + b'read-\n'
BEFORE_SECOND = BEFORE_FIRST + b'left\n'
INPUT_TIME = 1_000_000_000_000_000_000
WORKERS = ('left', 'right')
PRIMARY_BARRIERS = (
    b" Considering target file 'Makefile'.\n Still updating file 'Makefile'.\n",
    b" Considering target file 'Makefile'.\n File 'Makefile' was considered already.\n",
    b" Considering target file 'right.gen'.\n Still updating file 'right.gen'.\n",
)
WAIT_CHANNELS = {'core_sys_select', 'do_wait', '__x64_sys_poll'}
X86_64 = os.uname().machine == 'x86_64'

MISSING = b'Makefile:2: generated.gen: No such file or directory\n'
LEFT_ERROR = b'make: *** [Makefile:7: left] Error 3\n'
RIGHT_ERROR = b'make: *** [Makefile:8: right] Error 4\n'
WAITING = b'make: *** Waiting for unfinished jobs....\n'
REMAKE_ERROR = b"Makefile:2: Failed to remake makefile 'generated.gen'.\n"


LOAD_HIGH = b'0 0 0 99/100 1\n'
LOAD_LOW = b'0 0 0 0/100 1\n'
NAMED_LOAD_BARRIER = (b" Still updating file 'named.gen'.\n"
                      b" Considering target file 'right.gen'.\n"
                      b" Still updating file 'right.gen'.\n")
NAMED_LOAD_RIGHT_BARRIER = (b" File 'named.gen' was considered already.\n"
                            b" Considering target file 'right.gen'.\n"
                            b" Still updating file 'right.gen'.\n")


@dataclass(frozen=True)
class LoadProfile:
    mode: str
    at_release: bytes
    trace: bytes
    second_trace: bytes | None = None


@dataclass(frozen=True)
class HeldModel:
    model: debug.Model
    overlap: bool
    before_second: bytes | None
    before_first: bytes = BEFORE_FIRST
    before_second_stdout: bytes | None = None
    load: LoadProfile | None = None
    before_first_stdout: bytes = b''
    release_order: tuple[str, str] = WORKERS
    before_primary: bytes | None = None
    before_primary_stdout: bytes = b''
    fifo_controller: bool = False
    second_stdout_barrier: bytes | None = None
    tail_stdout_barrier: bytes | None = None
    named_recipe: bytes | None = None
    stock_primary: bool = False


def models(*, duplicate_reads: bool = False, include_load: bool = False) -> list[HeldModel]:
    worker = (SOURCES / 'worker.py').read_bytes()
    markers = {f'{name}.{suffix}': content for name in WORKERS
               for suffix, content in (('started', b'started\n'), ('release', b'yes\n'),
                                       ('done', b'done\n'))}
    if duplicate_reads:
        return duplicate_models(worker, markers)
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
    results += mixed_models(worker, markers)
    results += immediate_launch_models(worker, markers)
    results += shared_failure_models(worker, markers)
    results += duplicate_models(worker, markers)
    results += settled_mode_models(worker, markers)
    results += subdirectory_goal_models(worker, markers)
    results += inert_optional_mode_models(worker, markers)
    results += direct_optional_mode_models(worker, markers)
    for suffix, prerequisite in (('', ''), ('-normal-prerequisite', 'primer'),
                                 ('-order-only-prerequisite', '| primer')):
        results += grouped_optional_mode_models(worker, markers, suffix, prerequisite)
    results += named_source_mode_models(worker, markers)
    results += parallel_named_mode_models(worker, markers)
    results += primary_named_mode_models(worker, markers)
    results += primary_named_group_models(worker, markers)
    results += grouped_read_peer_question_models(worker, markers)
    results += later_primary_named_mode_models(worker, markers)
    results += forced_named_include_models(worker, markers)
    results += primary_named_implicit_models(worker, markers)
    results += named_include_file_prerequisite_models(worker, markers)
    results += named_include_file_prerequisite_models(worker, markers, optional_effect='secondary')
    results += named_include_file_prerequisite_models(worker, markers, optional_effect='left-file')
    results += combined_touch_question_models(results)
    results += primary_shared_include_models(worker, markers)
    results += primary_named_wait_models(worker, markers)
    results += carried_optional_mode_models(worker, markers)
    results += shuffled_remake_models(results)
    if include_load:
        direct_load = load_models(worker, markers)
        results += direct_load
        results += load_direct_build_mode_models(direct_load)
        results += load_goal_summary_models(worker, markers)
        results += load_build_mode_models(results, markers)
        results += named_load_models(results)
    return results


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


def duplicate_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    required = 'include left.gen\n'
    optional = '-include left.gen\n'
    right = 'include right.gen\n'
    first_error = b'Makefile:2: left.gen: No such file or directory\n'
    second_error = b'Makefile:3: left.gen: No such file or directory\n'
    left_error = b'make: *** [Makefile:7: left.gen] Error 3\n'
    missing = b"make: *** No rule to make target 'left.gen'."
    nested_missing = b"make: *** No rule to make target 'left-step', needed by 'left.gen'."
    cases = (
        ('success-required', required + required + right, b''),
        ('success-optional-required', optional + required + right, b''),
        ('success-optional-only', optional + optional + right, b''),
        ('fail-optional-required', optional + required + right,
         first_error + missing + b'  Stop.\n' + WAITING),
        ('fail-optional-required-keep', optional + required + right,
         first_error + missing + b"\nMakefile:3: Failed to remake makefile 'left.gen'.\n"),
        ('fail-optional-required-with-right-first', right + optional + required,
         second_error + missing + b'  Stop.\n' + WAITING),
        ('fail-required-optional-with-right-first', right + required + optional,
         left_error + WAITING),
        ('fail-required-required-with-right-first', right + required + required,
         second_error + left_error + WAITING),
        ('fail-optional-required-nested', optional + required + right,
         first_error + nested_missing + b'  Stop.\n' + WAITING),
        ('fail-optional-only', optional + optional + right, b''),
    )
    results = []
    for transport in ('pipe', 'fifo'):
        for name, directives, stderr in cases:
            success = name.startswith('success-')
            source = (
                '$(file >>seen,read-$(MAKE_RESTARTS))\n'
                + directives +
                '.PHONY: all force\n'
                'all: ; @$(file >>seen,goal-$(words $(filter left.gen,$(MAKEFILE_LIST)))-$(LEFT)-$(RIGHT)):\n'
            )
            inputs = {'worker.py': worker, 'seen': ANCHOR}
            updates = markers.copy()
            if success:
                source += (
                    'left.gen: ; @python3 worker.py left include\n'
                    'right.gen: ; @python3 worker.py right include\n'
                )
                updates.update({'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'})
                seen = BEFORE_SECOND + b'right\nread-1\ngoal-2-loaded-loaded\n'
                stdout, status = b'', 0
            else:
                if name.endswith('-nested'):
                    source += (
                        'left.gen: left-step ; @$(error failed prerequisite escaped)\n'
                        'left-step: ; @python3 worker.py left fail\n'
                    )
                else:
                    source += 'left.gen: ; @python3 worker.py left fail\n'
                source += 'right.gen: force ; @python3 worker.py right hold\n'
                inputs['right.gen'] = b'RIGHT := loaded\n'
                seen = BEFORE_SECOND + b'right\n'
                status = 2 if stderr else 0
                if not status:
                    seen += b'goal-0--loaded\n'
                stdout = b'left\n'
            inputs['Makefile'] = source.encode()
            args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}')
            args += ('-k', 'all') if name.endswith('-keep') else ('all',)
            expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                        'stderr_template_hex': stderr.hex()}
            model = debug.Model(f'duplicate-{name}-{transport}', inputs, args, {'GNUMAKEFLAGS': ''},
                                expected, seen, updates=updates, mtimes_ns=dict.fromkeys(inputs, INPUT_TIME))
            results.append(HeldModel(model, True, BEFORE_SECOND))
    return results + alias_models(worker, markers)


def alias_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    create_left = 'left.gen: ; @python3 worker.py left include\n'
    cases = (
        ('normalized-success', 'include ./left.gen\ninclude ././left.gen\ninclude right.gen\n',
         create_left + 'right.gen: ; @python3 worker.py right include\n', {},
         {'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'},
         b'read-1\nlist-Makefile left.gen left.gen right.gen\n', b''),
        ('normalized-optional-required-fail', '-include ./left.gen\ninclude left.gen\ninclude right.gen\n',
         'left.gen: ; @python3 worker.py left fail\nright.gen: force ; @python3 worker.py right hold\n',
         {'right.gen': b'# existing right\n'}, {}, b'',
         b'Makefile:2: left.gen: No such file or directory\n'
         b"make: *** No rule to make target 'left.gen'.  Stop.\n" + WAITING),
        ('dot-dot-distinct-records', 'include left.gen\ninclude aliases/../left.gen\n',
         create_left + 'aliases/../left.gen: ; @python3 worker.py right hold\n',
         {'aliases/anchor': b'parent remains reachable\n'}, {'left.gen': b'LEFT := loaded\n'},
         b'read-1\nlist-Makefile left.gen aliases/../left.gen\n', b''),
    )
    results = []
    for transport in ('pipe', 'fifo'):
        for name, includes, recipes, seeds, generated, after_remake, stderr in cases:
            source = (
                '$(file >>seen,read-$(MAKE_RESTARTS))\n'
                + includes
                + '.PHONY: all force\n'
                'all: ; @$(file >>seen,list-$(MAKEFILE_LIST)):\n'
                + recipes
            )
            files = {'Makefile': source.encode(), 'worker.py': worker, 'seen': ANCHOR, **seeds}
            args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}', 'all')
            expected = {
                'exit_code': 2 if stderr else 0,
                'stdout_template_hex': b'left\n'.hex() if stderr else '',
                'stderr_template_hex': stderr.hex(),
            }
            model = debug.Model(
                f'duplicate-{name}-{transport}', files, args, {'GNUMAKEFLAGS': ''}, expected,
                BEFORE_SECOND + b'right\n' + after_remake,
                updates={**markers, **generated}, mtimes_ns=dict.fromkeys(files, INPUT_TIME),
            )
            results.append(HeldModel(model, True, BEFORE_SECOND))
    return results


def settled_mode_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = '.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    ordinary = ('all: result\n'
                'result: ; @$(file >>seen,expanded-$(LEFT)-$(RIGHT))printf "ordinary\\n" > "$@"\n')
    independent = (
        'include left.gen right.gen\n'
        'left.gen: ; @python3 worker.py left include\n'
        'right.gen: ; @python3 worker.py right include\n'
    )
    named_prerequisite = (
        'include left.gen right.gen\n'
        'ifeq ($(MAKE_RESTARTS),)\n'
        'left.gen: named ; @:\n'
        'endif\n'
        'named: ; @python3 worker.py left include\n'
        'right.gen: ; @python3 worker.py right include\n'
    )
    required_failure = (
        'include generated.gen\n'
        'generated.gen: left right ; @$(error required failure must prevent this recipe)\n'
        'left: ; @python3 worker.py left fail\n'
        'right:\n'
        '\t@python3 worker.py right success\n'
        '\t@printf "right-tail\\n" >> seen\n'
    )
    required_error = (b'Makefile:3: generated.gen: No such file or directory\n'
                      b'make: *** [Makefile:5: left] Error 3\n')
    remake_error = b"Makefile:3: Failed to remake makefile 'generated.gen'.\n"
    loaded = {'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    restarted = BEFORE_SECOND + b'right\nread-1\n'
    loaded_effects = restarted + b'expanded-loaded-loaded\n'
    modes = (
        ('n', 0, b'printf "ordinary\\n" > "result"\n', loaded_effects, {}),
        ('q', 1, b'', loaded_effects, {}),
        ('t', 0, b'touch result\n', restarted, {'result': b''}),
    )
    named_stdout = {'n': b'python3 worker.py left include\n', 'q': b'', 't': b'touch named\n'}
    results = []
    for transport in ('pipe', 'fifo'):
        for mode, status, stdout, seen, updates in modes:
            cases = (
                ('success', independent, (), status, stdout, b'', seen, {**loaded, **updates}),
                ('explicit-goal', independent, ('./all',), status, stdout, b'', seen,
                 {**loaded, **updates}),
                ('named-prerequisite', named_prerequisite, ('named', 'all'), status,
                 named_stdout[mode] + stdout, b'', seen,
                 {**loaded, **updates, **({'named': b''} if mode == 't' else {})}),
                ('required-stop', required_failure, (), 2, b'left\n', required_error + WAITING,
                 BEFORE_SECOND + b'right\nright-tail\n', {}),
                ('required-keep', required_failure, ('-k',), 2, b'left\n', required_error + remake_error,
                 BEFORE_SECOND + b'right\nright-tail\n', {}),
            )
            for name, source, flags, exit_code, out, err, effects, extra_files in cases:
                files = {'Makefile': (header + source + ordinary).encode(),
                         'worker.py': worker, 'seen': ANCHOR}
                args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}',
                        '-' + mode, *flags)
                expected = {'exit_code': exit_code, 'stdout_template_hex': out.hex(),
                            'stderr_template_hex': err.hex()}
                model = debug.Model(f'settled-mode-{name}-{mode}-{transport}', files, args,
                                    {'GNUMAKEFLAGS': ''}, expected, effects,
                                    updates={**markers, **extra_files},
                                    mtimes_ns=dict.fromkeys(files, INPUT_TIME))
                results.append(HeldModel(model, True, BEFORE_SECOND,
                                         before_second_stdout=b'left\n' if name.startswith('required-') else b''))
    return results


def subdirectory_goal_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    loaded = {'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    restarted = BEFORE_SECOND + b'right\nread-1\n'
    results = []
    for transport in ('pipe', 'fifo'):
        for directive, role in (('include', 'required'), ('-include', 'optional')):
            for label, goal, operand, sources in (
                ('plain', 'build/result', 'build/result', 'left.gen right.gen'),
                ('normalized', 'obj/deep/result', './obj/deep/result', './left.gen ././right.gen'),
            ):
                source = (
                    '$(file >>seen,read-$(MAKE_RESTARTS))\n'
                    f'{directive} {sources}\n'
                    'left.gen: ; @python3 worker.py left include\n'
                    'right.gen: ; @python3 worker.py right include\n'
                    f'{goal}: ; @$(file >>seen,expanded-$(LEFT)-$(RIGHT))'
                    'printf "ordinary\\n" > "$@"\n'
                )
                parent = goal.rsplit('/', 1)[0]
                files = {'Makefile': source.encode(), 'worker.py': worker, 'seen': ANCHOR,
                         f'{parent}/anchor': b'keep directory\n'}
                expanded = restarted + b'expanded-loaded-loaded\n'
                for mode, status, stdout, seen, updates in (
                    ('n', 0, f'printf "ordinary\\n" > "{goal}"\n'.encode(), expanded, {}),
                    ('q', 1, b'', expanded, {}),
                    ('t', 0, f'touch {goal}\n'.encode(), restarted, {goal: b''}),
                ):
                    args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}',
                            f'-{mode}', operand)
                    expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                                'stderr_template_hex': ''}
                    model = debug.Model(
                        f'subdirectory-goal-{role}-{label}-{mode}-{transport}', files, args,
                        {'GNUMAKEFLAGS': ''}, expected, seen, updates={**markers, **loaded, **updates},
                        mtimes_ns=dict.fromkeys(files, INPUT_TIME),
                    )
                    results.append(HeldModel(model, True, BEFORE_SECOND, before_second_stdout=b''))
    return results


def inert_optional_mode_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = '$(file >>seen,read-$(MAKE_RESTARTS))\n'
    ordinary = ('all: result\n'
                'result: ; @$(file >>seen,ordinary-$(OPTIONAL))printf "ordinary\\n" > "$@"\n')
    optional = '-include optional.gen\n'
    independent = (
        'include left.gen right.gen\n'
        'left.gen: ; @python3 worker.py left include\n'
        'right.gen: ; @python3 worker.py right include\n'
    )
    controlled = (
        'include left.gen right.gen gate.gen\n{directive}'
        'ifeq ($(MAKE_RESTARTS),)\n'
        'left.gen: primer ; @python3 worker.py left {action}\n'
        'right.gen: primer ; @python3 worker.py right success\n'
        'gate.gen: primer ; @$(file >>seen,gate-expanded):\n'
        'endif\n'
    )
    generated = {'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    old_optional = {'optional.gen': b'OPTIONAL := old\n'}
    new_optional = {'optional.gen': b'OPTIONAL := new\n'}
    required = {'left.gen': b'# left current\n', 'right.gen': b'# right current\n',
                'gate.gen': b'# gate current\n', 'primer': b'primer\n'}
    changed = {'left.gen': b'# changed\n'}
    expanded_gate = BEFORE_FIRST + b'gate-expanded\n'
    cases = (
        ('present', 'Makefile', optional + independent, old_optional, generated,
         BEFORE_FIRST, True, b'old'),
        ('missing', 'Makefile', optional + independent, {}, generated,
         BEFORE_FIRST, True, b''),
        ('worker-write', 'Makefile', controlled.format(directive=optional, action='write-optional'),
         required, new_optional, expanded_gate, False, b''),
        ('parse-write-after-read', 'Makefile',
         controlled.format(directive=optional + '$(file >optional.gen,OPTIONAL := new)\n', action='success'),
         {**required, **old_optional}, new_optional, expanded_gate, False, b'old'),
        ('worker-write-restart', 'Makefile',
         controlled.format(directive=optional, action='write-optional-restart'),
         required, {**changed, **new_optional}, expanded_gate, True, b'new'),
        ('fallback-write-restart', 'preload.mk',
         controlled.format(directive='', action='write-fallback-restart'),
         required, {**changed, 'Makefile': b'OPTIONAL := new\n'}, expanded_gate, True, b'new'),
    )
    modes = (
        ('n', 0, b'printf "ordinary\\n" > "result"\n', {}),
        ('q', 1, b'', {}),
        ('t', 0, b'touch result\n', {'result': b''}),
    )
    results = []
    for transport in ('pipe', 'fifo'):
        for name, reader, source, initial, updates, checkpoint, restart, value in cases:
            files = {reader: (header + source + ordinary).encode(), 'worker.py': worker,
                     'seen': ANCHOR, **initial}
            times = dict.fromkeys(files, INPUT_TIME)
            if 'primer' in files:
                times['primer'] += 2_000_000_000
            environment = {'GNUMAKEFLAGS': ''}
            if reader == 'preload.mk':
                environment['MAKEFILES'] = reader
            before_second = checkpoint + b'left\n'
            seen = before_second + b'right\n'
            if restart:
                seen += b'read-1\n'
            for mode, status, stdout, ordinary_updates in modes:
                args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}',
                        '-' + mode, 'all')
                expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                            'stderr_template_hex': ''}
                effects = seen if mode == 't' else seen + b'ordinary-' + value + b'\n'
                model = debug.Model(f'inert-optional-mode-{name}-{mode}-{transport}', files, args,
                                    environment, expected, effects,
                                    updates={**markers, **updates, **ordinary_updates}, mtimes_ns=times)
                results.append(HeldModel(model, True, before_second, checkpoint, before_second_stdout=b''))
    return results


def direct_optional_mode_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = '.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    ordinary = ('all: result\n'
                'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT))printf "ordinary\\n" > "$@"\n')
    includes = '-include left.gen right.gen\n'
    success = (
        'left.gen: ; @python3 worker.py left include\n'
        'right.gen: ; @python3 worker.py right include\n'
    )
    failed = (
        'left.gen: ; @python3 worker.py left fail\n'
        'right.gen:\n'
        '\t@python3 worker.py right success\n'
        '\t@$(file >>seen,right-tail-expanded)printf "right-tail\\n" >> seen\n'
    )
    expanded = BEFORE_FIRST + b'right-tail-expanded\n'
    restarted = BEFORE_SECOND + b'right\nread-1\n'
    generated = {'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    modes = (
        ('n', 0, b'printf "ordinary\\n" > "result"\n', {}),
        ('q', 1, b'', {}),
        ('t', 0, b'touch result\n', {'result': b''}),
    )
    cases = (
        ('success', includes + success, (), BEFORE_FIRST, restarted, b'', b'loaded-loaded', generated),
        ('mixed-success', '-include left.gen\ninclude right.gen\n' + success, (),
         BEFORE_FIRST, restarted, b'', b'loaded-loaded', generated),
        ('failed', includes + failed, (), expanded,
         expanded + b'left\nright\nright-tail\n', b'left\n', b'-', {}),
        ('failed-keep', includes + failed, ('-k',), expanded,
         expanded + b'left\nright\nright-tail\n', b'left\n', b'-', {}),
    )
    results = []
    for transport in ('pipe', 'fifo'):
        for name, source, flags, before_first, seen, prefix, value, updates in cases:
            files = {'Makefile': (header + source + ordinary).encode(), 'worker.py': worker,
                     'seen': ANCHOR}
            for mode, status, stdout, ordinary_updates in modes:
                for builtins in (False, True):
                    flags_for_builtins = () if builtins else ('-rR',)
                    ident = f'direct-optional-mode-{name}-{mode}-{transport}'
                    if builtins:
                        ident += '-builtins'
                    args = (*flags_for_builtins, '--no-print-directory', '-j2', f'--jobserver-style={transport}',
                            '-' + mode, *flags, 'all')
                    expected = {'exit_code': status, 'stdout_template_hex': (prefix + stdout).hex(),
                                'stderr_template_hex': ''}
                    effects = seen if mode == 't' else seen + b'ordinary-' + value + b'\n'
                    model = debug.Model(ident, files, args,
                                        {'GNUMAKEFLAGS': ''}, expected, effects,
                                        updates={**markers, **updates, **ordinary_updates},
                                        mtimes_ns=dict.fromkeys(files, INPUT_TIME))
                    held = HeldModel(model, True, before_first + b'left\n', before_first,
                                     before_second_stdout=prefix)
                    results.append(held)
                    if not builtins and name.endswith('success'):
                        for label, function, marker in (
                            ('file', b'$(file >>seen,right-effect)', b'right-effect\n'),
                            ('shell', b'$(shell true)', b''),
                        ):
                            makefile = files['Makefile'].replace(
                                b'right.gen: ; @python3',
                                b'right.gen: ; @' + function + b'python3', 1)
                            first = before_first + marker
                            variant = replace(model, id=f'{ident}-{label}-effect',
                                              files={**files, 'Makefile': makefile},
                                              seen=effects.replace(before_first, first, 1))
                            results.append(replace(held, model=variant, before_first=first,
                                                   before_second=first + b'left\n'))
    return results


def grouped_optional_mode_models(worker: bytes, markers: dict[str, bytes],
                                 suffix: str, prerequisite: str) -> list[HeldModel]:
    header = '.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    ordinary = ('all: result\n'
                'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT)-$(PEER))'
                'printf "ordinary\\n" > "$@"\n')
    optional = '-include left.gen peer.gen\ninclude right.gen\n'
    required_peer = '-include left.gen\ninclude peer.gen right.gen\n'
    cases = (
        ('optional-peers', optional, False, False, False, b''),
        ('required-peer', required_peer, False, False, False, b''),
        ('right-first', 'include right.gen\n-include left.gen peer.gen\n',
         False, False, False, b''),
        ('optional-existing-peer', optional, False, True, False, b''),
        ('required-existing-peer', required_peer, False, True, False, b''),
        ('optional-failure', optional, True, False, False, b''),
        ('optional-failure-keep', optional, True, False, True, b''),
        ('required-peer-failure-keep', required_peer, True, False, True,
         b"Makefile:5: Failed to remake makefile 'peer.gen'.\n"),
    )
    modes = (
        ('n', 0, b'printf "ordinary\\n" > "result"\n', {}),
        ('q', 1, b'', {}),
        ('t', 0, b'touch result\n', {'result': b''}),
    )
    loaded = {'left.gen': b'LEFT := loaded\n', 'peer.gen': b'PEER := loaded\n',
              'right.gen': b'RIGHT := loaded\n'}
    restarted = BEFORE_SECOND + b'right\nread-1\n'
    results = []
    for transport in ('pipe', 'fifo'):
        for name, reads, failed, present, keep, stderr in cases:
            if failed:
                reads = 'ifeq ($(MAKE_RESTARTS),)\n' + reads + 'else\ninclude right.gen\nendif\n'
                recipe = f'left.gen peer.gen &: {prerequisite} ; @python3 worker.py left fail\n'
                prefix = b'left\n'
                value = b'--loaded-'
                updates = {'right.gen': loaded['right.gen']}
            else:
                recipe = (f'left.gen peer.gen &: {prerequisite} ; @python3 worker.py left include\n'
                          '\t@printf "PEER := loaded\\n" > peer.gen\n')
                prefix = b''
                value = b'-loaded-loaded-loaded'
                updates = loaded
            source = header + reads + recipe + 'right.gen: ; @python3 worker.py right include\n' + ordinary
            files = {'Makefile': source.encode(), 'worker.py': worker, 'seen': ANCHOR}
            if present:
                files['peer.gen'] = b'PEER := initial\n'
            if prerequisite:
                files['primer'] = b'primer\n'
            times = dict.fromkeys(files, INPUT_TIME)
            if prerequisite:
                times['primer'] += 500_000_000
            for mode, status, stdout, ordinary_updates in modes:
                flags = ('-k',) if keep else ()
                args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}',
                        '-' + mode, *flags, 'all')
                expected = {'exit_code': status, 'stdout_template_hex': (prefix + stdout).hex(),
                            'stderr_template_hex': stderr.hex()}
                seen = restarted if mode == 't' else restarted + b'ordinary' + value + b'\n'
                ident = f'grouped-optional-mode-{name}-{mode}-{transport}{suffix}'
                model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected, seen,
                                    updates={**markers, **updates, **ordinary_updates},
                                    mtimes_ns=times)
                results.append(HeldModel(model, True, BEFORE_SECOND, before_second_stdout=prefix))
    return results


def named_source_mode_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = '.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    rules = (
        'left.gen: ; @python3 worker.py left include\n'
        'right.gen: ; @python3 worker.py right include\n'
        'all: result\n'
        'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT))printf "ordinary\\n" > "$@"\n'
    )
    right = {name: content for name, content in markers.items() if name.startswith('right.')}
    right['right.gen'] = b'RIGHT := loaded\n'
    read = BEFORE_FIRST + b'right\n'
    restarted = read + b'read-1\n'
    dry_command = b'python3 worker.py left include\n'
    touch_command = b'touch left.gen\n'
    current = b"make: 'left.gen' is up to date.\n"
    ordinary = b'printf "ordinary\\n" > "result"\n'
    missing = b"make: *** No rule to make target 'left.gen'."
    modes = (
        ('dry', '-n', False, 0, dry_command, dry_command + current + ordinary, b'',
         restarted + b'ordinary--loaded\n', {}),
        ('query', '-q', False, 1, b'', b'', b'', read + b'ordinary--\n', {}),
        ('touch', '-t', False, 0, touch_command, current + b'touch result\n', b'',
         restarted, {'left.gen': b'', 'result': b''}),
    )
    results = []
    for transport in ('pipe', 'fifo'):
        for directive in ('include', '-include'):
            cases = modes if directive == 'include' else (modes[0], modes[2],
                ('query', '-q', False, 2, b'', b'', missing + b'  Stop.\n', read, {}),
                ('query-keep', '-q', True, 1, b'', b'', missing + b'\n',
                 read + b'ordinary--\n', {}))
            files = {'Makefile': (header + directive + ' left.gen right.gen\n' + rules).encode(),
                     'worker.py': worker, 'seen': ANCHOR}
            policy = 'required' if directive == 'include' else 'optional'
            for name, flag, keep, status, prefix, suffix, stderr, seen, updates in cases:
                args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}', flag)
                if keep:
                    args += ('-k',)
                expected = {'exit_code': status, 'stdout_template_hex': (prefix + suffix).hex(),
                            'stderr_template_hex': stderr.hex()}
                model = debug.Model(f'named-source-mode-{policy}-{name}-{transport}', files,
                                    (*args, './left.gen', 'all'), {'GNUMAKEFLAGS': ''}, expected,
                                    seen, updates={**right, **updates},
                                    mtimes_ns=dict.fromkeys(files, INPUT_TIME))
                results.append(HeldModel(model, False, None, before_first_stdout=prefix,
                                         release_order=('right', 'left')))
    return results


def parallel_named_mode_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = '.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    rules = (
        'named.gen: ; @printf "VALUE := rebuilt\\n" > named.gen\n'
        'left.gen: ; @python3 worker.py left include\n'
        'right.gen:\n'
        '\t@python3 worker.py right include\n'
        '\t@$(file >>seen,tail-expanded)printf "tail\\n" >> seen\n'
        'all: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT)-$(VALUE))printf "ordinary\\n"\n'
    )
    first = BEFORE_FIRST + b'tail-expanded\n'
    second = first + b'left\n'
    completed = second + b'right\ntail\n'
    generated = {'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    dry_command = b'printf "VALUE := rebuilt\\n" > named.gen\n'
    current = b"make: 'named.gen' is up to date.\n"
    modes = (
        ('n', 0, dry_command, dry_command + current + b'printf "ordinary\\n"\n',
         completed + b'read-1\nordinary-loaded-loaded-\n', {}),
        ('q', 1, b'', b'', completed + b'ordinary---\n', {}),
        ('t', 0, b'touch named.gen\n', current + b'touch all\n',
         completed + b'read-1\n', {'named.gen': b'', 'all': b''}),
    )
    orders = (
        ('first', 'named.gen left.gen right.gen'),
        ('middle', 'left.gen named.gen right.gen'),
        ('last', 'left.gen right.gen named.gen'),
    )
    results = []
    for transport in ('pipe', 'fifo'):
        for name, sources in orders:
            files = {'Makefile': (header + f'include {sources}\n' + rules).encode(),
                     'worker.py': worker, 'seen': ANCHOR}
            for slots in (2, 3):
                for mode, status, prefix, suffix, seen, updates in modes:
                    args = ('-rR', '--no-print-directory', f'-j{slots}', f'--jobserver-style={transport}',
                            '-' + mode, './named.gen', 'all')
                    expected = {'exit_code': status, 'stdout_template_hex': (prefix + suffix).hex(),
                                'stderr_template_hex': ''}
                    ident = f'parallel-named-mode-{name}-{mode}-{transport}-{slots}-slots'
                    model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected, seen,
                                        updates={**markers, **generated, **updates},
                                        mtimes_ns=dict.fromkeys(files, INPUT_TIME))
                    first_stdout = b'' if name == 'last' and mode == 'n' and slots == 2 else prefix
                    results.append(HeldModel(model, True, second, first,
                                             before_second_stdout=prefix, before_first_stdout=first_stdout))

            failed = dict(files, Makefile=files['Makefile'].replace(b'worker.py left include',
                                                                   b'worker.py left fail'))
            args = ('-rR', '--no-print-directory', '-j3', f'--jobserver-style={transport}',
                    '-q', '-k', './named.gen', 'all')
            stderr = (b'Makefile:3: right.gen: No such file or directory\n'
                      b'make: *** [Makefile:5: left.gen] Error 3\n')
            expected = {'exit_code': 1, 'stdout_template_hex': b'left\n'.hex(),
                        'stderr_template_hex': stderr.hex()}
            model = debug.Model(f'parallel-named-mode-{name}-query-keep-failure-{transport}', failed,
                                args, {'GNUMAKEFLAGS': ''}, expected, completed + b'ordinary---\n',
                                updates={**markers, 'right.gen': generated['right.gen']},
                                mtimes_ns=dict.fromkeys(failed, INPUT_TIME))
            results.append(HeldModel(model, True, second, first, before_second_stdout=b'left\n'))
    return results


def primary_named_mode_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    source = (
        '.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
        'include left.gen right.gen\n'
        'left.gen: ; @python3 worker.py left include\n'
        'right.gen:\n'
        '\t@python3 worker.py right include\n'
        '\t@$(file >>seen,tail-expanded)printf "tail\\n" >> seen\n'
        'all: result\n'
        'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT))printf "ordinary\\n" > "$@"\n'
    )
    first = BEFORE_FIRST + b'tail-expanded\n'
    second = first + b'left\n'
    completed = second + b'right\ntail\n'
    restarted = completed + b'read-1\n'
    loaded = restarted + b'ordinary-loaded-loaded\n'
    generated = {'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    current = b"make: 'Makefile' is up to date.\n"
    unruled = b"make: Nothing to be done for 'Makefile'.\n"
    append = b'printf "# remade\\n" >> "Makefile"\n'
    append_recipe = b'printf "# remade\\n" >> "$@"\n'
    child_args = b' --no-print-directory -f child.mk append\n'
    ordinary = b'printf "ordinary\\n" > "result"\n'
    results = []
    for transport in ('pipe', 'fifo'):
        for discovery, slots in (('explicit', 3), ('explicit', 2), ('default', 3)):
            recipes = [('plain', 'primer', b'@' + append_recipe)]
            if discovery == 'explicit':
                recipes += [('order-only', '| primer', b'@' + append_recipe),
                            ('forced', 'primer', b'+@' + append_recipe),
                            ('recursive', 'primer', b'@$(MAKE)' + child_args)]
            variants = [(style, prerequisites, recipe, age)
                        for style, prerequisites, recipe in recipes for age in ('current', 'stale')]
            if (discovery, slots) == ('explicit', 3):
                variants.append(('plain', '', b'', 'unruled'))
            for style, prerequisites, recipe, age in variants:
                ruled = bool(recipe)
                stale = age == 'stale'
                active = stale and style != 'order-only'
                forced = style in ('forced', 'recursive')
                makefile = source.encode()
                if ruled:
                    makefile += f'Makefile: {prerequisites} ; '.encode() + recipe
                files = {'Makefile': makefile, 'worker.py': worker, 'seen': ANCHOR}
                if ruled:
                    files['primer'] = b'primer\n'
                if style == 'recursive':
                    files['child.mk'] = b'append: ; +@' + append
                times = dict.fromkeys(files, INPUT_TIME)
                if ruled:
                    times['primer'] += 2_000_000_000 if stale else -2_000_000_000
                summary = current if ruled else unruled
                dry_prefix = (b'make' + child_args if style == 'recursive' else append) if active else b''
                simulated = active and not forced
                touch_prefix = b'touch Makefile\n' if simulated else b''
                changes = {'Makefile': makefile + b'# remade\n'} if active and forced else {}
                cases = (
                    ('n', 0, dry_prefix, (dry_prefix if simulated else b'') + summary + ordinary, loaded, {}),
                    ('q', 1, b'', b'', completed + b'ordinary--\n' if simulated else loaded, {}),
                    ('t', 0, touch_prefix, summary + b'touch result\n', restarted, {'result': b''}),
                )
                for mode, status, prefix, suffix, seen, updates in cases:
                    args = ('-rR', '--no-print-directory', f'-j{slots}', f'--jobserver-style={transport}',
                            '-' + mode)
                    if discovery == 'explicit':
                        args += ('-f', './Makefile')
                    if style == 'recursive':
                        args += ('MAKE=make',)
                    args += ('./Makefile', 'all')
                    expected = {'exit_code': status, 'stdout_template_hex': (prefix + suffix).hex(),
                                'stderr_template_hex': ''}
                    ident = f'primary-named-mode-{discovery}-{age}-{mode}-{transport}-{slots}-slots'
                    if style != 'plain':
                        ident += f'-{style}'
                    model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected, seen,
                                        updates={**markers, **generated, **changes, **updates}, mtimes_ns=times)
                    held = HeldModel(model, True, second, first,
                                     before_second_stdout=prefix, before_first_stdout=prefix)
                    results.append(held)
                    if discovery == 'explicit' and slots == 3 and ruled and style == 'plain':
                        results.append(named_secondary_variant(held, BEFORE_FIRST))
    return results


def named_secondary_variant(held: HeldModel, expansion_point: bytes,
                            target: bytes = b'Makefile', *,
                            effect: tuple[str, bytes] | None = None) -> HeldModel:
    model = held.model
    source = model.files['Makefile']
    original = target + b': primer ; '
    if original not in source:
        original = target + b': ; '
    assert original in source
    suffix, expression = effect or ('secondary', b'$$(file >>seen,secondary-$$@-$$(MAKE_RESTARTS))')
    secondary = (b'.SECONDEXPANSION:\nINPUT := primer\n'
                 + target + b': ' + expression + b' $$(INPUT) ; ')
    marker = b'secondary-' + target + b'-'

    def record_expansion(seen: bytes) -> bytes:
        if effect is not None:
            return seen
        seen = seen.replace(expansion_point, expansion_point + marker + b'\n', 1)
        return seen.replace(b'read-1\n', b'read-1\n' + marker + b'1\n', 1)

    files = {**model.files, 'Makefile': source.replace(original, secondary, 1)}
    times = model.mtimes_ns
    if 'primer' not in files:
        files['primer'] = b'base input\n'
        times = {**times, 'primer': INPUT_TIME}
    updates = model.updates.copy()
    if 'Makefile' in updates:
        updates['Makefile'] = updates['Makefile'].replace(original, secondary, 1)
    model = replace(model, id=f'{model.id}-{suffix}', files=files, seen=record_expansion(model.seen),
                    updates=updates, mtimes_ns=times)
    assert held.before_second is not None
    return replace(held, model=model, before_first=record_expansion(held.before_first),
                   before_second=record_expansion(held.before_second))


def primary_named_group_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = (
        b'.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
        b'include left.gen right.gen\n'
    )
    command = b'printf "# remade\\n" >> Makefile; printf "peer\\n" > peer.out\n'
    rules = (
        b'left.gen: ; @python3 worker.py left include\n'
        b'right.gen:\n'
        b'\t@python3 worker.py right include\n'
        b'\t@$(file >>seen,tail-expanded)printf "tail\\n" >> seen\n'
        b'all: result\n'
        b'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT))printf "ordinary\\n" > "$@"\n'
    )
    first = BEFORE_FIRST + b'tail-expanded\n'
    second = first + b'left\n'
    completed = second + b'right\ntail\n'
    loaded = completed + b'read-1\nordinary-loaded-loaded\n'
    generated = {**markers, 'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    ordinary = b'printf "ordinary\\n" > "result"\n'
    def record_expansions(seen: bytes) -> bytes:
        first_read = BEFORE_FIRST + b'secondary-Makefile-\nsecondary-peer.out-\n'
        reread = b'read-1\nsecondary-Makefile-1\nsecondary-peer.out-1\n'
        return seen.replace(BEFORE_FIRST, first_read, 1).replace(b'read-1\n', reread, 1)

    results = []
    for order, targets in (('primary-first', b'Makefile peer.out'),
                           ('peer-first', b'peer.out Makefile')):
        source = header + targets + b' &: primer ; @' + command + rules
        secondary_source = (
            b'.SECONDEXPANSION:\nINPUT := primer\n' + source.replace(
                b' &: primer ;',
                b' &: $$(file >>seen,secondary-$$@-$$(MAKE_RESTARTS)) $$(INPUT) ;', 1)
        )
        for age, peer_exists, stale in (('current', True, False), ('stale', True, True),
                                        ('missing-peer', False, False)):
            files = {'Makefile': source, 'worker.py': worker, 'seen': ANCHOR,
                     'primer': b'primer\n'}
            if peer_exists:
                files['peer.out'] = b'peer\n'
            times = dict.fromkeys(files, INPUT_TIME)
            times['primer'] += 2_000_000_000 if stale else -2_000_000_000
            outdated = stale or not peer_exists
            for named_peer in (False, True):
                goals = ('./Makefile', 'peer.out', 'all') if named_peer else ('./Makefile', 'all')
                summary = b"make: 'Makefile' is up to date.\n"
                if named_peer:
                    summary += b"make: 'peer.out' is up to date.\n"
                dry_prefix = command if outdated else b''
                cases = (
                    ('n', 0, dry_prefix, dry_prefix * 2 + summary + ordinary, loaded),
                    ('q', 1, b'', b'', completed + b'ordinary--\n' if outdated else loaded),
                )
                for mode, status, prefix, stdout, seen in cases:
                    for transport in ('pipe', 'fifo'):
                        args = ('-rR', '--no-print-directory', '-j3',
                                f'--jobserver-style={transport}', '-' + mode, '-f', './Makefile',
                                *goals)
                        ident = f'primary-named-group-{order}-{age}-{mode}-{transport}'
                        if named_peer:
                            ident += '-peer-goal'
                        expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                                    'stderr_template_hex': ''}
                        model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected,
                                            seen, updates=generated, mtimes_ns=times)
                        held = HeldModel(model, True, second, first,
                                         before_second_stdout=prefix, before_first_stdout=prefix)
                        results.append(held)
                        secondary = replace(model, id=ident + '-secondary',
                                            files={**files, 'Makefile': secondary_source},
                                            seen=record_expansions(seen))
                        results.append(replace(held, model=secondary,
                                               before_first=record_expansions(first),
                                               before_second=record_expansions(second)))
    return results


def grouped_read_peer_question_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = b'.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    workers = b'include left.gen right.gen\n'
    rules = (
        b'Makefile peer.gen &: primer ; @printf "# remade\\n" >> Makefile; '
        b'printf "PEER := loaded\\n" > peer.gen\n'
        b'left.gen: ; @python3 worker.py left include\n'
        b'right.gen:\n'
        b'\t@python3 worker.py right include\n'
        b'\t@$(file >>seen,tail-expanded)printf "tail\\n" >> seen\n'
        b'all: result\n'
        b'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT)-$(PEER))printf "ordinary\\n" > "$@"\n'
    )
    first = BEFORE_FIRST + b'tail-expanded\n'
    second = first + b'left\n'
    completed = second + b'right\ntail\n'
    generated = {**markers, 'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    results = []
    for policy, directive in (('required', b'include '), ('optional', b'-include ')):
        peer_read = directive + b'peer.gen\n'
        for order, reads in (('first', peer_read + workers), ('last', workers + peer_read)):
            source = header + reads + rules
            for age in ('current', 'stale', 'missing'):
                files = {'Makefile': source, 'worker.py': worker, 'seen': ANCHOR,
                         'primer': b'primer\n'}
                if age != 'missing':
                    files['peer.gen'] = b'PEER := initial\n'
                times = dict.fromkeys(files, INPUT_TIME)
                times['primer'] += 2_000_000_000 if age == 'stale' else -2_000_000_000
                updates = generated
                if age == 'current':
                    seen = completed + b'read-1\nordinary-loaded-loaded-initial\n'
                else:
                    value = b'initial' if age == 'stale' else b''
                    seen = completed + b'ordinary---' + value + b'\n'
                    updates = {**generated, 'Makefile': source + b'# remade\n',
                               'peer.gen': b'PEER := loaded\n'}
                for transport in ('pipe', 'fifo'):
                    args = ('-rR', '--no-print-directory', '-j3', f'--jobserver-style={transport}',
                            '-q', '-f', './Makefile', './Makefile', 'all')
                    ident = f'grouped-read-peer-{policy}-{order}-{age}-q-{transport}'
                    expected = {'exit_code': 1, 'stdout_template_hex': '', 'stderr_template_hex': ''}
                    model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected, seen,
                                        updates=updates, mtimes_ns=times)
                    results.append(HeldModel(model, True, second, first))
    return results


def later_primary_named_mode_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = b'.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    rules = (
        b'left.gen: ; @python3 worker.py left include\n'
        b'right.gen:\n'
        b'\t@python3 worker.py right include\n'
        b'\t@$(file >>seen,tail-expanded)printf "tail\\n" >> seen\n'
        b'all: result\n'
        b'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT))printf "ordinary\\n" > "$@"\n'
    )
    first = BEFORE_FIRST + b'tail-expanded\n'
    second = first + b'left\n'
    workers_done = second + b'right\n'
    current = b"make: 'Makefile' is up to date.\n"
    ordinary = b'printf "ordinary\\n" > "result"\n'
    recurse = b'make --no-print-directory -f child.mk append\n'
    append = b'printf "# remade\\n" >> "Makefile"\n'
    recipes = (
        ('forced', b'+@printf "# remade\\n" >> "$@"\n', append, {}),
        ('recursive', b'@$(MAKE) --no-print-directory -f child.mk append\n', recurse,
         {'child.mk': b'append: ; @printf "child\\n" > child.txt\n'}),
    )
    results = []
    for transport in ('pipe', 'fifo'):
        for slots, sync in ((2, False), (3, False), (3, True)):
            for style, recipe, command, child_files in recipes:
                if sync and style == 'recursive':
                    continue
                makefile = header + b'Makefile: primer ; ' + recipe + rules
                files = {'Makefile': makefile, 'earlier.mk': b'include left.gen right.gen\n',
                         'worker.py': worker, 'seen': ANCHOR, 'primer': b'primer\n', **child_files}
                for stale in (False, True):
                    times = dict.fromkeys(files, INPUT_TIME)
                    times['primer'] += 2_000_000_000 if stale else -2_000_000_000
                    saturated = stale and slots == 2
                    complete = workers_done if saturated else workers_done + b'tail\n'
                    restarted = complete + b'read-1\n'
                    loaded = restarted + b'ordinary-loaded-loaded\n'
                    changes = {}
                    if stale:
                        changes = ({'Makefile': makefile + b'# remade\n'} if style == 'forced'
                                   else {'child.txt': b'child\n'})
                    for mode in 'nqt':
                        stdout = stderr = first_stdout = second_stdout = b''
                        updates = {**markers, **changes, 'left.gen': b'LEFT := loaded\n'}
                        if mode != 'q' or not saturated:
                            updates['right.gen'] = b'RIGHT := loaded\n'
                        seen = loaded
                        if mode == 'n':
                            if stale:
                                first_stdout = b'' if saturated else command
                                second_stdout = stdout = command
                                if saturated:
                                    stdout += b'printf "tail\\n" >> seen\n'
                                if style == 'recursive':
                                    stdout += command
                            stdout += current + ordinary
                        elif mode == 'q' and saturated:
                            seen = workers_done + b'ordinary--\n'
                            stderr = b"make: *** Deleting file 'right.gen'\n"
                        elif mode == 't':
                            seen = restarted
                            if stale:
                                second_stdout = stdout = b'touch left.gen\n'
                            if saturated:
                                stdout += b'touch right.gen\ntouch right.gen\n'
                            stdout += current + b'touch result\n'
                            updates['result'] = b''
                        args = ('-rR', '--no-print-directory', f'-j{slots}', f'--jobserver-style={transport}')
                        if sync:
                            args += ('-Oline',)
                        args += ('-' + mode, '-f', 'earlier.mk', '-f', './Makefile')
                        if style == 'recursive':
                            args += ('MAKE=make',)
                        args += ('./Makefile', 'all')
                        age = 'stale' if stale else 'current'
                        ident = f'later-primary-named-mode-{style}-{age}-{mode}-{transport}-{slots}-slots'
                        if sync:
                            ident += '-sync'
                        expected = {'exit_code': int(mode == 'q'), 'stdout_template_hex': stdout.hex(),
                                    'stderr_template_hex': stderr.hex()}
                        model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected, seen,
                                            updates=updates, mtimes_ns=times)
                        held = HeldModel(model, True, second, first,
                                         before_second_stdout=second_stdout, before_first_stdout=first_stdout)
                        results.append(held)
                        if slots == 3 and not sync and style == 'forced':
                            results.append(named_secondary_variant(held, first))
    return results


def forced_named_include_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = '.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    rules = (
        'named.gen: ; +@printf "NAMED := loaded\\n" > "$@"\n'
        'left.gen: ; @python3 worker.py left include\n'
        'right.gen:\n'
        '\t@python3 worker.py right include\n'
        '\t@$(file >>seen,tail-expanded)printf "tail\\n" >> seen\n'
        'all: result\n'
        'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT)-$(NAMED))printf "ordinary\\n" > "$@"\n'
    )
    first = BEFORE_FIRST + b'tail-expanded\n'
    second = first + b'left\n'
    completed = second + b'right\n'
    command = b'printf "NAMED := loaded\\n" > "named.gen"\n'
    summary = b"make: 'named.gen' is up to date.\n"
    deletion = b"make: *** Deleting file 'right.gen'\n"
    left_touch = b'touch left.gen\n'
    both = {**markers, 'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    results = []
    for transport in ('pipe', 'fifo'):
        for slots in (2, 3):
            base = ('-rR', '--no-print-directory', f'-j{slots}', f'--jobserver-style={transport}')
            for directive in ('include', '-include'):
                policy = 'required' if directive == 'include' else 'optional'
                for position, order in (
                    ('first', 'named.gen left.gen right.gen'),
                    ('middle', 'left.gen named.gen right.gen'),
                    ('last', 'left.gen right.gen named.gen'),
                ):
                    source = (header + f'{directive} {order}\n' + rules).encode()
                    files = {'Makefile': source, 'worker.py': worker, 'seen': ANCHOR}
                    times = dict.fromkeys(files, INPUT_TIME)
                    saturated = position == 'last' and slots == 2
                    seen = completed + (b'' if saturated else b'tail\n') + b'read-1\n'
                    loaded = seen + b'ordinary-loaded-loaded-loaded\n'
                    written = {**both, 'named.gen': b'NAMED := loaded\n'}
                    dry = command
                    if saturated:
                        dry += b'printf "tail\\n" >> seen\n'
                    dry += summary + b'printf "ordinary\\n" > "result"\n'
                    touch_prefix = left_touch if position == 'last' or policy == 'optional' else b''
                    touch = touch_prefix
                    if saturated:
                        touch += b'touch right.gen\ntouch right.gen\n'
                    touch += summary + b'touch result\n'
                    query_updates = written.copy()
                    if saturated:
                        del query_updates['right.gen']
                    cases = (
                        ('n', 0, dry, b'', loaded, written, b'' if saturated else command, command),
                        ('q', 1, b'', deletion if saturated else b'',
                         completed + b'ordinary---\n' if saturated else loaded, query_updates, b'', b''),
                        ('t', 0, touch, b'', seen, {**written, 'result': b''}, b'', touch_prefix),
                    )
                    for mode, status, stdout, stderr, effects, updates, first_out, second_out in cases:
                        ident = f'forced-named-include-{policy}-{position}-{mode}-{transport}-{slots}-slots'
                        args = (*base, '-' + mode, './named.gen', 'all')
                        expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                                    'stderr_template_hex': stderr.hex()}
                        model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected, effects,
                                            updates=updates, mtimes_ns=times)
                        held = HeldModel(model, True, second, first,
                                         before_second_stdout=second_out, before_first_stdout=first_out)
                        ordered_completion = policy == 'optional' and saturated
                        expansion_point = first if position == 'last' else BEFORE_FIRST
                        direct = order_named_completion(held) if ordered_completion else held
                        results.append(direct)
                        if slots == 2:
                            results.append(named_secondary_variant(direct, expansion_point, b'named.gen'))
                        if ordered_completion and mode in ('n', 't'):
                            results.extend(right_first_named_models(held, expansion_point, mode))
                        if slots == 3:
                            source_with_primer = source.replace(b'named.gen: ;', b'named.gen: primer ;')
                            with_primer = {**files, 'Makefile': source_with_primer, 'primer': b'base input\n'}
                            prerequisite = replace(model, id=ident + '-prerequisite', files=with_primer,
                                                   mtimes_ns={**times, 'primer': INPUT_TIME})
                            with_prerequisite = replace(held, model=prerequisite)
                            results.append(with_prerequisite)
                            results.append(named_secondary_variant(
                                with_prerequisite, expansion_point, b'named.gen'))
                        if position == 'last':
                            duplicate_source = source.replace(f'{directive} {order}\n'.encode(),
                                                              f'{directive} {order} ./named.gen\n'.encode(), 1)
                            duplicate = replace(model, id=ident + '-duplicate',
                                                files={**files, 'Makefile': duplicate_source})
                            repeated = replace(held, model=duplicate)
                            if ordered_completion and mode in ('n', 't'):
                                results.extend(right_first_named_models(repeated, expansion_point, mode))
                            if ordered_completion:
                                repeated = order_named_completion(repeated)
                            if policy == 'optional' and mode == 'q' and saturated:
                                right_first = replace(
                                    duplicate, id=duplicate.id + '-right-first',
                                    expected={**duplicate.expected, 'stderr_template_hex': ''},
                                    seen=completed + b'tail\nread-1\nordinary-loaded-loaded-loaded\n',
                                    updates=written,
                                )
                                right_first = order_named_completion(
                                    replace(held, model=right_first), after='right')
                                results.append(right_first)
                                for keep_going in (False, True):
                                    results.append(earlier_named_question(
                                        right_first, duplicate, keep_going=keep_going))
                            results.append(repeated)
                            results.append(named_secondary_variant(repeated, expansion_point, b'named.gen'))

            failed_source = (header + 'include left.gen right.gen named.gen\n' + rules).encode()
            failed_source = failed_source.replace(b'+@printf "NAMED := loaded\\n" > "$@"', b'+@false')
            files = {'Makefile': failed_source, 'worker.py': worker, 'seen': ANCHOR}
            expected = {'exit_code': 1, 'stdout_template_hex': '',
                        'stderr_template_hex': (WAITING + deletion).hex()}
            model = debug.Model(f'forced-named-include-question-stop-{transport}-{slots}-slots', files,
                                (*base, '-q', './named.gen', 'all'), {'GNUMAKEFLAGS': ''}, expected,
                                completed, updates={**markers, 'left.gen': b'LEFT := loaded\n'},
                                mtimes_ns=dict.fromkeys(files, INPUT_TIME))
            results.append(HeldModel(model, True, second, first, before_second_stdout=b''))
    return results


def order_named_completion(held: HeldModel, *, after: str | None = None) -> HeldModel:
    model = held.model
    named_recipe = b'+@printf "NAMED := loaded\\n" > "$@"'
    named_command = b'+@python3 named_reap.py'
    makefile = model.files['Makefile']
    worker = model.files['worker.py']
    if after == 'tail':
        named_command += b' after-right-tail'
        tail = b'printf "tail\\n" >> seen'
        assert makefile.count(tail) == 1
        makefile = makefile.replace(tail, b'python3 named_reap.py write-right-tail')
    elif after == 'right':
        named_command += b' after-right'
        anchor = b'name, mode = sys.argv[1:]'
        assert worker.count(anchor) == 1
        worker = worker.replace(anchor, anchor + b"\nif name == 'right':\n"
                                b"    from named_reap import record_process\n"
                                b"    record_process('right.pid')")
    else:
        anchor = b"if release.read_bytes() != b'yes\\n':\n    raise SystemExit(96)\n"
        assert worker.count(anchor) == 1
        worker = worker.replace(anchor, anchor + b"\nif name == 'right':\n"
                                b"    from named_reap import wait_for_reap\n"
                                b"    wait_for_reap('named.pid')\n")
    assert makefile.count(named_recipe) == 1
    files = {**model.files, 'Makefile': makefile.replace(named_recipe, named_command),
             'worker.py': worker, 'named_reap.py': (SOURCES / 'named_reap.py').read_bytes()}
    echoed_recipe = named_recipe.removeprefix(b'+@').replace(b'$@', b'named.gen') + b'\n'
    echoed_command = named_command.removeprefix(b'+@') + b'\n'
    stdout = bytes.fromhex(model.expected['stdout_template_hex'])
    expected = {**model.expected,
                'stdout_template_hex': stdout.replace(echoed_recipe, echoed_command).hex()}
    model = replace(model, files=files, mtimes_ns={**model.mtimes_ns, 'named_reap.py': INPUT_TIME},
                    environment={**model.environment, 'PYTHONDONTWRITEBYTECODE': '1'},
                    expected=expected)
    assert held.before_second_stdout is not None
    return replace(held, model=model,
                   before_first_stdout=held.before_first_stdout.replace(echoed_recipe, echoed_command),
                   before_second_stdout=held.before_second_stdout.replace(echoed_recipe, echoed_command))


def right_first_named_models(held: HeldModel, expansion_point: bytes,
                             mode: str) -> tuple[HeldModel, HeldModel]:
    model = held.model
    tail = b'printf "tail\\n" >> seen\n' if mode == 'n' else b'touch right.gen\n' * 2
    stdout = bytes.fromhex(model.expected['stdout_template_hex'])
    assert stdout.count(tail) == model.seen.count(b'right\n') == 1
    expected = {**model.expected, 'stdout_template_hex': stdout.replace(tail, b'').hex()}
    model = replace(model, id=model.id + '-right-first', expected=expected,
                    seen=model.seen.replace(b'right\n', b'right\ntail\n'))
    ordered = order_named_completion(replace(held, model=model), after='right' if mode == 'n' else 'tail')
    return ordered, named_secondary_variant(ordered, expansion_point, b'named.gen')


def earlier_named_question(held: HeldModel, original: debug.Model, *, keep_going: bool) -> HeldModel:
    model = held.model
    source = model.files['Makefile']
    read = b'$(file >>seen,read-$(MAKE_RESTARTS))\n'
    prior = b'-include prior.gen\nprior.gen: ; @printf "prior\\n" > "$@"\n'
    assert source.count(read) == 1
    flags = ('-k',) if keep_going else ()
    args = (*model.args[:-2], *flags, './prior.gen', *model.args[-2:])
    stderr = bytes.fromhex(original.expected['stderr_template_hex'])
    stderr += b"make: *** No rule to make target 'prior.gen'."
    stderr += b'\n' if keep_going else b'  Stop.\n'
    expected = {**model.expected, 'exit_code': 1 if keep_going else 2,
                'stderr_template_hex': stderr.hex()}
    suffix = '-keep-earlier-question' if keep_going else '-earlier-question'
    seen = original.seen if keep_going else original.seen.removesuffix(b'ordinary---\n')
    model = replace(model, id=model.id + suffix, args=args, expected=expected, seen=seen,
                    files={**model.files, 'Makefile': source.replace(read, read + prior)},
                    updates=original.updates)
    return replace(held, model=model)


def primary_named_implicit_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = (
        b'.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
        b'include left.gen right.gen\n'
    )
    rules = (
        b'left.gen: ; @python3 worker.py left include\n'
        b'right.gen:\n'
        b'\t@python3 worker.py right include\n'
        b'\t@$(file >>seen,tail-expanded)printf "tail\\n" >> seen\n'
        b'all: result\n'
        b'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT))printf "ordinary\\n" > "$@"\n'
    )
    primary = b'Makefile: primer ; @printf "# remade\\n" >> "$@"\n'
    first = BEFORE_FIRST + b'tail-expanded\n'
    second = first + b'left\n'
    completed = second + b'right\ntail\n'
    restarted = completed + b'read-1\n'
    loaded = restarted + b'ordinary-loaded-loaded\n'
    current = b"make: 'Makefile' is up to date.\n"
    no_rule = b"make: Nothing to be done for 'Makefile'.\n"
    ordinary = b'printf "ordinary\\n" > "result"\n'
    append = b'printf "# remade\\n" >> "Makefile"\n'
    touch = b'touch Makefile\n'
    primer = {'primer': b'primer\n'}
    cases = (
        ('leaf', primary, primer, append, touch, (False, True)),
        ('builtin-prerequisite', primary, {**primer, 'primer.c': b'int main(void) { return 0; }\n'},
         b'cc     primer.c   -o primer\n' + append, b'touch primer\n' + touch, (False, True)),
        ('pattern-prerequisite', b'%mer: %mer.source ; @cp "$<" "$@"\n' + primary,
         {**primer, 'primer.source': b'rebuilt\n'}, b'cp "primer.source" "primer"\n' + append,
         b'touch primer\n' + touch, (False, True)),
        ('unruled', b'', {}, b'', b'', (False,)),
        ('implicit-primary', b'%akefile: %.template ; @cp "$<" "$@"\n',
         {'M.template': b'# newer template\n'}, b'cp "M.template" "Makefile"\n', touch, (False,)),
    )
    generated = {**markers, 'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    results = []
    for transport in ('pipe', 'fifo'):
        for name, primary_rule, extra, dry_command, touch_command, ages in cases:
            files = {'Makefile': header + primary_rule + rules, 'worker.py': worker, 'seen': ANCHOR,
                     **extra}
            summary = no_rule if name == 'unruled' else current
            for stale_primer in ages:
                times = dict.fromkeys(files, INPUT_TIME)
                if 'primer' in files:
                    times['primer'] += 2_000_000_000 if stale_primer else -2_000_000_000
                implicit_inputs = extra.keys() - {'primer'}
                for filename in implicit_inputs:
                    times[filename] += 3_000_000_000
                stale = stale_primer or bool(implicit_inputs)
                dry_prefix = dry_command if stale else b''
                touch_prefix = touch_command if stale else b''
                modes = (
                    ('n', 0, dry_prefix, dry_prefix + summary + ordinary, loaded, {}),
                    ('q', 1, b'', b'', completed + b'ordinary--\n' if stale else loaded, {}),
                    ('t', 0, touch_prefix, summary + b'touch result\n', restarted, {'result': b''}),
                )
                for mode, status, prefix, suffix, seen, updates in modes:
                    args = ('--no-print-directory', '-j3', f'--jobserver-style={transport}',
                            '-' + mode, '-f', './Makefile', './Makefile', 'all')
                    age = 'newer-primer' if stale_primer else 'older-or-absent-primer'
                    ident = f'primary-named-implicit-{name}-{age}-{mode}-{transport}'
                    expected = {'exit_code': status, 'stdout_template_hex': (prefix + suffix).hex(),
                                'stderr_template_hex': ''}
                    model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected, seen,
                                        updates={**generated, **updates}, mtimes_ns=times)
                    results.append(HeldModel(model, True, second, first,
                                             before_second_stdout=prefix, before_first_stdout=prefix))
    return results


def named_include_file_prerequisite_models(worker: bytes, markers: dict[str, bytes], *,
                                           optional_effect: str | None = None) -> list[HeldModel]:
    header = '.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    rules = (
        'named.gen: primer ; @printf "NAMED := loaded\\n" > "$@"\n'
        'left.gen: ; @python3 worker.py left include\n'
        'right.gen:\n'
        '\t@python3 worker.py right include\n'
        '\t@$(file >>seen,tail-expanded)printf "tail\\n" >> seen\n'
        'all: result\n'
        'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT)-$(NAMED))printf "ordinary\\n" > "$@"\n'
    )
    command = b'printf "NAMED := loaded\\n" > "named.gen"\n'
    summary = b"make: 'named.gen' is up to date.\n"
    ordinary = b'printf "ordinary\\n" > "result"\n'
    touch_named = b'touch named.gen\n'
    missing = b"make: *** No rule to make target 'named.gen'.  Stop.\n"
    generated = {**markers, 'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    results = []
    for transport in ('pipe', 'fifo'):
        for directive in ('include', '-include'):
            optional = directive == '-include'
            if optional_effect is not None and not optional:
                continue
            policy = 'optional' if optional else 'required'
            for position, order in (
                ('first', 'named.gen left.gen right.gen'),
                ('middle', 'left.gen named.gen right.gen'),
                ('last', 'left.gen right.gen named.gen'),
            ):
                if optional_effect == 'left-file' and position != 'middle':
                    continue
                source = (header + f'{directive} {order}\n' + rules).encode()
                before_tail = BEFORE_FIRST
                if optional_effect == 'left-file':
                    source = source.replace(b'left.gen: ; @python3',
                                            b'left.gen: ; @$(file >>seen,left-file)python3', 1)
                    before_tail += b'left-file\n'
                files = {'Makefile': source, 'worker.py': worker, 'seen': ANCHOR, 'primer': b'base input\n'}
                for mode in 'nqt':
                    split_pass = (optional and position == 'middle' and mode != 'q'
                                  and optional_effect is None)
                    first = before_tail if split_pass else before_tail + b'tail-expanded\n'
                    second = first + (b'left\ntail-expanded\n' if split_pass else b'left\n')
                    completed = second + b'right\ntail\n'
                    stderr = first_out = second_out = b''
                    status = 0
                    updates = generated
                    if mode == 'n':
                        first_out = second_out = command
                        stdout = command + command + summary + ordinary
                        seen = completed + b'read-1\nordinary-loaded-loaded-\n'
                    elif mode == 'q':
                        status = 2 if optional else 1
                        stderr = missing if optional else b''
                        stdout = b''
                        seen = completed if optional else completed + b'ordinary---\n'
                    else:
                        first_out = touch_named
                        second_out = touch_named + (b'touch left.gen\n' if split_pass else b'')
                        stdout = second_out + summary + b'touch result\n'
                        seen = completed + b'read-1\n'
                        updates = {**generated, 'named.gen': b'', 'result': b''}
                    args = ('-rR', '--no-print-directory', '-j3', f'--jobserver-style={transport}',
                            '-' + mode, './named.gen', 'all')
                    ident = f'named-include-file-prerequisite-{policy}-{position}-{mode}-{transport}'
                    if optional_effect == 'left-file':
                        ident += '-earlier-file-write'
                    expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                                'stderr_template_hex': stderr.hex()}
                    model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected, seen,
                                        updates=updates, mtimes_ns=dict.fromkeys(files, INPUT_TIME))
                    held = HeldModel(model, not split_pass, second, first,
                                     before_second_stdout=second_out, before_first_stdout=first_out)
                    expansion_point = first if position == 'last' else BEFORE_FIRST
                    if optional_effect != 'secondary':
                        results.append(held)
                    if not optional or optional_effect == 'secondary':
                        results.append(named_secondary_variant(held, expansion_point, b'named.gen'))
                    if optional and position == 'middle':
                        if optional_effect == 'secondary':
                            effects = (('secondary-shell-success', b'$$(shell true)'),
                                       ('secondary-shell-nonzero', b'$$(shell false)'))
                        elif optional_effect is None:
                            effects = (('secondary-shell-empty', b'$$(shell )'),
                                       ('secondary-eval', b'$$(eval EXTRA = value)'))
                        else:
                            effects = ()
                        for effect in effects:
                            results.append(named_secondary_variant(
                                held, expansion_point, b'named.gen', effect=effect))
                    if optional_effect == 'left-file':
                        continue
                    duplicate_source = source.replace(f'{directive} {order}\n'.encode(),
                                                      f'{directive} {order} ./named.gen\n'.encode(), 1)
                    duplicate_source = duplicate_source.replace(b'named.gen: primer ;', b'named.gen: ;')
                    duplicate_files = {'Makefile': duplicate_source, 'worker.py': worker, 'seen': ANCHOR}
                    touches_earlier = optional and position == 'last' and mode == 't'
                    duplicate_second = second_out + b'touch left.gen\n' if touches_earlier else second_out
                    duplicate_expected = expected
                    if touches_earlier:
                        duplicate_stdout = duplicate_second + stdout[len(second_out):]
                        duplicate_expected = {**expected, 'stdout_template_hex': duplicate_stdout.hex()}
                    for slots in (2, 3):
                        duplicate_args = (*args[:2], f'-j{slots}', *args[3:])
                        duplicate_ident = f'duplicate-named-include-{position}-{mode}-{transport}-{slots}-slots'
                        if optional:
                            duplicate_ident += '-optional'
                        duplicate = replace(model, args=duplicate_args, files=duplicate_files,
                                            id=duplicate_ident, expected=duplicate_expected,
                                            mtimes_ns=dict.fromkeys(duplicate_files, INPUT_TIME))
                        waits_for_slot = mode == 'n' and position == 'last' and slots == 2
                        repeated = HeldModel(duplicate, not split_pass, second, first,
                                             before_second_stdout=duplicate_second,
                                             before_first_stdout=b'' if waits_for_slot else first_out)
                        if optional_effect == 'secondary':
                            repeated = named_secondary_variant(repeated, expansion_point, b'named.gen')
                        results.append(repeated)
    return results


def combined_touch_question_models(held_models: list[HeldModel]) -> list[HeldModel]:
    by_id = {held.model.id: held for held in held_models}
    summary = b"make: 'named.gen' is up to date.\n"
    touch_named = b'touch named.gen\n'
    prior_error = (
        b"Makefile:3: prior.gen: No such file or directory\n"
        b"make: *** No rule to make target 'missing-prerequisite', needed by 'prior.gen'.\n"
        b"Makefile:3: Failed to remake makefile 'prior.gen'.\n"
    )
    first = BEFORE_FIRST + b'tail-expanded\n'
    second = first + b'left\n'
    restarted = second + b'right\ntail\nread-1\n'
    results = []
    for policy in ('required', 'optional'):
        for position in ('first', 'middle', 'last'):
            for transport in ('pipe', 'fifo'):
                name = f'named-include-file-prerequisite-{policy}-{position}-t-{transport}'
                held = by_id[name]
                model = held.model
                base = tuple(arg for arg in model.args if arg != '-t')
                stdout = bytes.fromhex(model.expected['stdout_template_hex']).replace(summary, b'')
                for order, modes in (('qt', ('-q', '-t')), ('tq', ('-t', '-q'))):
                    ident = f'{name}-combined-{order}'
                    args = (*base[:-1], *modes, base[-1])
                    variant = replace(model, id=ident, args=args,
                                      expected={**model.expected, 'stdout_template_hex': stdout.hex()})
                    results.append(replace(held, model=variant))
                    if policy != 'optional' or position != 'middle':
                        continue
                    for earlier, directive in (('optional', b'-include'), ('required', b'include')):
                        required = earlier == 'required'
                        line = b'-include left.gen named.gen right.gen\n'
                        makefile = model.files['Makefile'].replace(
                            line, directive + b' prior.gen\n' + line, 1)
                        makefile += b'prior.gen: missing-prerequisite\n'
                        before = (*args[:-1], '-k', args[-1]) if required else args
                        updates = model.updates.copy()
                        if required:
                            del updates['result']
                        expected = {
                            'exit_code': 2 if required else 0,
                            'stdout_template_hex': (touch_named + (b'' if required else b'touch result\n')).hex(),
                            'stderr_template_hex': (prior_error * 2 if required else b'').hex(),
                        }
                        prior = replace(variant, id=f'{ident}-prior-{earlier}', args=before,
                                        files={**model.files, 'Makefile': makefile},
                                        expected=expected, seen=restarted, updates=updates)
                        results.append(HeldModel(prior, True, second, first,
                                                 before_first_stdout=touch_named,
                                                 before_second_stdout=touch_named))
    return results


def primary_shared_include_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = b'.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    rules = (
        b'left.gen: ; @python3 worker.py left include\n'
        b'right.gen:\n'
        b'\t@python3 worker.py right include\n'
        b'\t@$(file >>seen,tail-expanded)printf "tail\\n" >> seen\n'
        b'all: result\n'
        b'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT))printf "ordinary\\n" > "$@"\n'
    )
    first = BEFORE_FIRST + b'tail-expanded\n'
    second = first + b'left\n'
    completed = second + b'right\n'
    restarted = completed + b'read-1\n'
    loaded = restarted + b'ordinary-loaded-loaded\n'
    append = b'printf "# remade\\n" >> "Makefile"\n'
    tail = b'printf "tail\\n" >> seen\n'
    current = b"make: 'Makefile' is up to date.\n"
    ordinary = b'printf "ordinary\\n" > "result"\n'
    deletion = b"make: *** Deleting file 'right.gen'\n"
    left_touch = b'touch left.gen\n'
    right_touch = b'touch right.gen\ntouch right.gen\n'
    primary_touch = b'touch Makefile\n'
    left = {**markers, 'left.gen': b'LEFT := loaded\n'}
    both = {**left, 'right.gen': b'RIGHT := loaded\n'}
    results = []
    for transport in ('pipe', 'fifo'):
        for slots, dotted in ((2, False), (3, False), (3, True)):
            include = b'include ./left.gen ./right.gen\n' if dotted else b'include left.gen right.gen\n'
            for dependency in WORKERS:
                on_left = dependency == 'left'
                primary = f'Makefile: {dependency}.gen ; @printf "# remade\\n" >> "$@"\n'.encode()
                files = {'Makefile': header + primary + rules, 'earlier.mk': include,
                         'worker.py': worker, 'seen': ANCHOR}
                dry = append + tail if on_left else tail + append
                touched = left_touch + (primary_touch + right_touch if on_left else right_touch + primary_touch)
                cases = (
                    ('n', 0, dry + append + current + ordinary, b'', loaded, both, append if on_left else b''),
                    ('q', 1, b'', deletion, completed + b'ordinary--\n', left, b''),
                    ('t', 0, touched + current + b'touch result\n', b'', restarted,
                     {**both, 'result': b''}, left_touch + (primary_touch if on_left else b'')),
                )
                for mode, status, stdout, stderr, seen, updates, second_out in cases:
                    args = ('-rR', '--no-print-directory', f'-j{slots}', f'--jobserver-style={transport}',
                            '-' + mode, '-f', 'earlier.mk', '-f', './Makefile', './Makefile', 'all')
                    spelling = 'dot' if dotted else 'plain'
                    ident = f'primary-shared-include-{dependency}-{mode}-{transport}-{slots}-slots-{spelling}'
                    expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                                'stderr_template_hex': stderr.hex()}
                    model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected, seen,
                                        updates=updates, mtimes_ns=dict.fromkeys(files, INPUT_TIME))
                    results.append(HeldModel(model, True, second, first, before_second_stdout=second_out))
    return results


def primary_named_wait_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = b'.DEFAULT_GOAL := all\n$(file >>seen,read-$(MAKE_RESTARTS))\n'
    include = b'include left.gen right.gen\n'
    rules = (
        b'left.gen: ; @python3 worker.py left include\n'
        b'right.gen:\n'
        b'\t@python3 worker.py right include\n'
        b'\t@$(file >>seen,tail-expanded)printf "tail\\n" >> seen\n'
        b'all: result\n'
        b'result: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT))printf "ordinary\\n" > "$@"\n'
    )
    recipe = b' ; @printf "# remade %s/%s\\n" "$^" "$|" >> "$@"\n'
    command = b'printf "# remade %s/%s\\n" '
    current = b"make: 'Makefile' is up to date.\n"
    ordinary = b'printf "ordinary\\n" > "result"\n'
    tail = b'printf "tail\\n" >> seen\n'
    touch_primary = b'touch Makefile\n'
    touch_result = current + b'touch result\n'
    touch_left = b'touch left.gen\n'
    touch_right = b'touch right.gen\ntouch right.gen\n'
    deletion = b"make: *** Deleting file 'right.gen'\n"
    first = BEFORE_FIRST + b'tail-expanded\n'
    second = first + b'left\n'
    generated_left = {**markers, 'left.gen': b'LEFT := loaded\n'}
    generated_both = {**generated_left, 'right.gen': b'RIGHT := loaded\n'}
    shapes = (
        ('local-normal', b'older .WAIT primer', False, b'older primer', False),
        ('local-order-only', b'| older .WAIT primer', False, b'', False),
        ('local-later', b'older .WAIT primer', True, b'older primer', False),
        ('shared-left-right', b'left.gen .WAIT right.gen', True, b'left.gen right.gen', True),
        ('shared-right-left', b'right.gen .WAIT left.gen', True, b'right.gen left.gen', True),
        ('shared-left-file', b'left.gen .WAIT primer', True, b'left.gen primer', True),
        ('shared-file-right', b'older .WAIT right.gen', True, b'older right.gen', True),
    )
    def record_secondary(seen: bytes, point: bytes) -> bytes:
        return seen.replace(point, point + b'secondary-Makefile-\n', 1).replace(
            b'read-1\n', b'read-1\nsecondary-Makefile-1\n', 1)

    results = []
    for name, prerequisites, later, normal, shared in shapes:
        source = header + (b'' if later else include) + b'Makefile: ' + prerequisites + recipe + rules
        deferred_clause = (b'.SECONDEXPANSION:\n'
                           b'Makefile: $$(file >>seen,secondary-$$@-$$(MAKE_RESTARTS))\n')
        secondary_source = source.replace(b'Makefile: ', deferred_clause + b'Makefile: ', 1)
        expansion_point = first if later else BEFORE_FIRST
        files = {'Makefile': source, 'worker.py': worker, 'seen': ANCHOR,
                 'older': b'older\n', 'primer': b'primer\n'}
        if later:
            files['earlier.mk'] = include
        append = command + b'"' + normal + b'" "" >> "Makefile"\n'
        completed = second + (b'right\n' if shared else b'right\ntail\n')
        restarted = completed + b'read-1\n'
        loaded = restarted + b'ordinary-loaded-loaded\n'
        for newer in ((False,) if shared else (False, True)):
            times = dict.fromkeys(files, INPUT_TIME)
            times['older'] -= 2_000_000_000
            times['primer'] += 500_000_000 if newer else -1_000_000_000
            stale = newer and bool(normal)
            if shared:
                early = name == 'shared-left-file'
                dry = (append + tail if early else tail + append) + append
                touched = touch_left + (touch_primary + touch_right if early else touch_right + touch_primary)
                cases = (
                    ('n', 0, b'', append if early else b'', dry + current + ordinary,
                     b'', loaded, generated_both),
                    ('q', 1, b'', b'', b'', deletion, completed + b'ordinary--\n', generated_left),
                    ('t', 0, b'', touch_left + (touch_primary if early else b''),
                     touched + touch_result, b'', restarted, {**generated_both, 'result': b''}),
                )
            else:
                dry = append if stale else b''
                touched = touch_primary if stale else b''
                cases = (
                    ('n', 0, dry, dry, dry * 2 + current + ordinary, b'', loaded, generated_both),
                    ('q', 1, b'', b'', b'', b'',
                     completed + b'ordinary--\n' if stale else loaded, generated_both),
                    ('t', 0, touched, touched, touched + touch_result,
                     b'', restarted, {**generated_both, 'result': b''}),
                )
            for mode, status, first_out, second_out, stdout, stderr, seen, updates in cases:
                for transport in ('pipe', 'fifo'):
                    args = ('-rR', '--no-print-directory', '-j3', f'--jobserver-style={transport}', '-' + mode)
                    if later:
                        args += ('-f', 'earlier.mk')
                    args += ('-f', './Makefile', './Makefile', 'all')
                    age = 'newer' if newer else 'older'
                    ident = f'primary-named-wait-{name}-{mode}-{transport}-{age}'
                    expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                                'stderr_template_hex': stderr.hex()}
                    model = debug.Model(ident, files, args, {'GNUMAKEFLAGS': ''}, expected, seen,
                                        updates=updates, mtimes_ns=times)
                    held = HeldModel(model, True, second, first,
                                     before_second_stdout=second_out, before_first_stdout=first_out)
                    results.append(held)
                    secondary = replace(model, id=ident + '-secondary',
                                        files={**files, 'Makefile': secondary_source},
                                        seen=record_secondary(seen, expansion_point))
                    results.append(replace(held, model=secondary,
                                           before_first=record_secondary(first, expansion_point),
                                           before_second=record_secondary(second, expansion_point)))
    return results


def carried_optional_mode_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = (
        '.DEFAULT_GOAL := all\n'
        '$(file >>seen,read-$(MAKE_RESTARTS))\n'
        '-include generated.gen\n'
        'generated.gen: left right ; @$(error failed dependencies must prevent this recipe)\n'
        'left: ; @python3 worker.py left fail\n'
        'right: ; @python3 worker.py right success\n'
    )
    tail = '\t@$(file >>seen,right-tail-expanded)printf "right-tail\\n" >> seen\n'
    ordinary = ('all: result\n'
                'result: ; @$(file >>seen,ordinary)printf "ordinary\\n" > "$@"\n')
    dry = b'printf "ordinary\\n" > "result"\n'
    modes = (('n', 0, dry), ('q', 1, b''), ('t', 0, b'touch result\n'))
    profiles = (
        ('stop-tail', False, True, ('', 'target', 'line', 'recurse')),
        ('keep-tail', True, True, ('', 'target')),
        ('stop-single', False, False, ('',)),
        ('keep-single', True, False, ('',)),
    )
    results = []
    for transport in ('pipe', 'fifo'):
        for name, keep, has_tail, output_modes in profiles:
            files = {'Makefile': (header + (tail if has_tail else '') + ordinary).encode(),
                     'worker.py': worker, 'seen': ANCHOR}
            first = BEFORE_FIRST + (b'right-tail-expanded\n' if has_tail else b'')
            for mode, status, ordinary_stdout in modes:
                second = first + b'left\n'
                stdout = b'left\n' + ordinary_stdout
                before_second_stdout = b'left\n' if keep else stdout
                stderr = WAITING if mode == 'q' and not keep else b''
                updates = markers.copy()
                if keep:
                    seen = second + b'right\n'
                    if has_tail:
                        seen += b'right-tail\n'
                    if mode != 't':
                        seen += b'ordinary\n'
                else:
                    if mode != 't':
                        second += b'ordinary\n'
                    seen = second + b'right\n'
                    if mode == 'n' and has_tail:
                        stdout += b'printf "right-tail\\n" >> seen\n'
                    if mode == 't':
                        updates['right'] = b''
                        stdout += b'touch right\n' * (2 if has_tail else 1)
                if mode == 't':
                    updates['result'] = b''
                for sync in output_modes:
                    args = ('-rR', '--no-print-directory', '-j2', f'--jobserver-style={transport}',
                            '-' + mode)
                    if keep:
                        args += ('-k',)
                    if sync:
                        args += ('-O' + sync,)
                    expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                                'stderr_template_hex': stderr.hex()}
                    ident = f'carried-optional-mode-{name}-{mode}-{transport}-{sync or "none"}'
                    model = debug.Model(ident, files, (*args, 'all'), {'GNUMAKEFLAGS': ''},
                                        expected, seen, updates=updates,
                                        mtimes_ns=dict.fromkeys(files, INPUT_TIME))
                    results.append(HeldModel(model, True, second, first,
                                             before_second_stdout=before_second_stdout))
                    if name == 'stop-tail' and mode == 'q' and not sync:
                        # Expansion output identifies the final query visit
                        # without advancing the file-effect cursor.
                        info = b'ordinary-query-visited\n'
                        info_files = {**files, 'Makefile': files['Makefile'].replace(
                            b'$(file >>seen,ordinary)', b'$(info ordinary-query-visited)')}
                        info_model = replace(
                            model, id=f'{ident}-without-file-write', files=info_files,
                            expected={**expected, 'stdout_template_hex': (stdout + info).hex()},
                            seen=seen.replace(b'ordinary\n', b''))
                        results.append(HeldModel(
                            info_model, True, second.removesuffix(b'ordinary\n'), first,
                            before_second_stdout=before_second_stdout + info))
    return results


def load_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    header = '$(file >>seen,read-$(MAKE_RESTARTS))\n'
    ordinary = ('.PHONY: all\n'
                'all: ; @$(file >>seen,ordinary-$(LEFT)-$(RIGHT)-$(TAIL)):\n')
    ordinary_expansion = '.PHONY: all\n.SECONDEXPANSION:\nall: $$(file >>seen,ordinary---)\n'
    left = 'left.gen: ; @$(file >>seen,expanded-left)python3 worker.py left include\n'
    right = 'right.gen: ; @$(file >>seen,expanded-right)python3 worker.py right include\n'
    independent = 'include left.gen right.gen\n' + left + right + ordinary
    dynamic = ('include left.gen right.gen tail.gen\n' + left + right
               + 'tail.gen: ; @$(file >>seen,expanded-tail)printf "TAIL := loaded\\n" > "$@"\n'
               + ordinary)
    blocked = (
        'generated.gen: left right ; @$(error failed prerequisites must block this recipe)\n'
        'left: ; @$(file >>seen,expanded-left)python3 worker.py left fail\n'
        'right: ; @$(file >>seen,expanded-right)python3 worker.py right success\n'
    )
    expanded = BEFORE_FIRST + b'expanded-left\nexpanded-right\n'
    after_left = expanded + b'left\n'
    dynamic_expanded = expanded + b'expanded-tail\n'
    expanded_goal = b'ordinary---\n'
    right_effect = b'right\n'
    loaded = {'left.gen': b'LEFT := loaded\n', 'right.gen': b'RIGHT := loaded\n'}
    restarted = right_effect + b'read-1\nordinary-loaded-loaded-\n'
    ordinary_summary = b"make: Nothing to be done for 'all'.\n"
    required_error = MISSING + b'make: *** [Makefile:4: left] Error 3\n'
    left_markers = {name: data for name, data in markers.items() if name.startswith('left.')}
    high = LoadProfile('normal', LOAD_HIGH, b'proc\n')
    low = LoadProfile('normal', LOAD_LOW, b'proc\n')
    drop = LoadProfile('drop-on-token-wait', LOAD_LOW, b'proc\ntoken-wait\n')
    cases = (
        ('queued-success', independent, (), False, LOAD_HIGH, high,
         expanded, after_left, after_left + restarted, 0, b'', b'', {**markers, **loaded}),
        ('low-success', independent, (), True, LOAD_LOW, low,
         expanded, after_left, after_left + restarted, 0, b'', b'', {**markers, **loaded}),
        ('dynamic-token-wait', dynamic, (), True, LOAD_HIGH, drop,
         dynamic_expanded, dynamic_expanded + b'left\n',
         dynamic_expanded + b'left\nright\nread-1\nordinary-loaded-loaded-loaded\n',
         0, b'', b'', {**markers, **loaded, 'tail.gen': b'TAIL := loaded\n', 'load.input': LOAD_LOW}),
        ('optional-stop', '-include generated.gen\n' + blocked + ordinary_expansion, (),
         False, LOAD_HIGH, high, expanded, after_left + expanded_goal,
         after_left + expanded_goal + right_effect, 0, b'left\n' + ordinary_summary, b'', markers),
        ('optional-keep', '-include generated.gen\n' + blocked + ordinary_expansion, ('-k',),
         False, LOAD_HIGH, high, expanded, after_left,
         after_left + right_effect + expanded_goal, 0, b'left\n' + ordinary_summary, b'', markers),
        ('required-stop', 'include generated.gen\n' + blocked + ordinary_expansion, (),
         False, LOAD_HIGH, high, expanded, None, after_left, 2, b'left\n', required_error, left_markers),
    )
    results = []
    for transport in ('pipe', 'fifo'):
        for (name, source, flags, overlap, initial_load, load, before_first,
             before_second, seen, status, stdout, stderr, updates) in cases:
            files = {'Makefile': (header + source).encode(), 'worker.py': worker,
                     'seen': ANCHOR, 'load.input': initial_load}
            args = ('-rR', '--no-print-directory', '-j2', '-l2',
                    f'--jobserver-style={transport}', *flags, 'all')
            expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                        'stderr_template_hex': stderr.hex()}
            model = debug.Model(f'load-{name}-{transport}', files, args, {'GNUMAKEFLAGS': ''},
                                expected, seen, updates=updates,
                                mtimes_ns=dict.fromkeys(files, INPUT_TIME))
            results.append(HeldModel(model, overlap, before_second, before_first,
                                     before_second_stdout=stdout if name == 'optional-stop' else None,
                                     load=load))
    return results


def load_direct_build_mode_models(available: list[HeldModel]) -> list[HeldModel]:
    modes = (
        ('n', 0, b':\n'),
        ('q', 1, b''),
        ('t', 0, b"make: Nothing to be done for 'all'.\n"),
    )
    results = []
    for held in available:
        model = held.model
        if not model.id.startswith(('load-queued-success-', 'load-low-success-',
                                    'load-dynamic-token-wait-')):
            continue
        for mode, status, stdout in modes:
            seen = model.seen
            if mode == 't':
                seen = seen[:seen.rindex(b'ordinary-')]
            expected = {**model.expected, 'exit_code': status, 'stdout_template_hex': stdout.hex()}
            args = (*model.args[:-1], f'-{mode}', model.args[-1])
            active_mode = replace(model, id=f'{model.id}-{mode}', args=args,
                                  expected=expected, seen=seen)
            results.append(replace(held, model=active_mode, before_second_stdout=b''))
    return results


def named_load_models(available: list[HeldModel]) -> list[HeldModel]:
    dry_named = b'printf "VALUE := rebuilt\\n" > named.gen\n'
    dry_right = b'python3 worker.py right include\n'
    dry_tail = b'printf "tail\\n" >> seen\n'
    rebuilt = b'right\ntail\nread-1\nordinary-loaded-loaded-rebuilt\n'
    results = []
    for held in available:
        base = held.model
        named_source = base.id.startswith('named-source-mode-')
        parallel_named = base.id.startswith('parallel-named-mode-') and base.id.endswith('-2-slots')
        if not (named_source or parallel_named):
            continue
        if parallel_named:
            position, mode, _, _, _ = base.id.removeprefix('parallel-named-mode-').split('-')
        for tag, load_bytes in (('high', LOAD_HIGH), ('low', LOAD_LOW)):
            files = {**base.files, 'load.input': load_bytes}
            model = replace(base, id=f'load-{base.id}-{tag}', files=files,
                            args=(*base.args[:2], '-l2', *base.args[2:]),
                            mtimes_ns=dict.fromkeys(files, INPUT_TIME))
            load = LoadProfile('normal', load_bytes, b'proc\n')
            if named_source:
                # Its only live worker does not need a load sample.
                results.append(replace(held, model=model, load=replace(load, trace=b'')))
                continue

            high = tag == 'high'
            stdout = bytes.fromhex(base.expected['stdout_template_hex'])
            seen, updates = base.seen, base.updates
            first_stdout = held.before_first_stdout
            before_second, second_stdout = held.before_second, held.before_second_stdout
            if high and position == 'middle' and mode in ('n', 'q'):
                # The named recipe executes with the queued ordinary remake
                # visit and is already current when the requested mode resumes.
                seen = before_second + rebuilt
                updates = {**updates, 'named.gen': b'VALUE := rebuilt\n'}
                stdout = stdout.removeprefix(dry_named * 2)
                first_stdout = second_stdout = b''
            elif high and position == 'last':
                if mode == 'n':
                    # The right worker starts only after a restart. Its tail
                    # and the named recipe are simulated again on that pass.
                    second_stdout = dry_right + dry_tail + dry_named
                    before_second += b'read-1\ntail-expanded\n'
                    seen = before_second + b'right\nread-2\nordinary-loaded-loaded-\n'
                    stdout = second_stdout + dry_tail + stdout
                    load = replace(load, second_trace=b'proc\nproc\n')
                elif mode == 'q':
                    # The query verdict retires the queued right recipe.
                    seen = before_second + b'ordinary---\n'
                    updates = {name: value for name, value in updates.items()
                               if not name.startswith('right.')}
                    before_second = second_stdout = None
            expected = {**model.expected, 'stdout_template_hex': stdout.hex()}
            model = replace(model, expected=expected, seen=seen, updates=updates)
            required = replace(held, model=model, load=load, overlap=not high,
                               before_first_stdout=first_stdout, before_second=before_second,
                               before_second_stdout=second_stdout)
            optional = optional_named_load_model(required, position, mode, high)
            results.extend((required, optional, directory_named_load_model(required),
                            directory_named_load_model(optional), builtin_load_model(required, explicit=True),
                            builtin_load_model(optional, explicit=True), builtin_load_model(required),
                            builtin_load_model(optional)))
            results.append(skipped_required_named_load_model(held, required, high))
    results.extend(stock_primary_load_models())
    return results


def skipped_required_named_load_model(base: HeldModel, required: HeldModel, high: bool) -> HeldModel:
    model = required.model
    files = {**model.files, 'Makefile': model.files['Makefile'] + b'.PHONY: named.gen\n'}
    updates = {name: value for name, value in base.model.updates.items()
               if name.startswith(('left.', 'right.'))}
    missing = b'Makefile:3: named.gen: No such file or directory\n'
    expected = {'exit_code': 2, 'stdout_template_hex': '', 'stderr_template_hex': missing.hex()}
    model = replace(model, id=model.id.replace('load-parallel-', 'load-phony-parallel-', 1),
                    files=files, expected=expected, seen=base.before_second + b'right\ntail\n',
                    updates=updates)
    load = LoadProfile('normal', LOAD_HIGH if high else LOAD_LOW, b'proc\n')
    return HeldModel(model, not high, base.before_second, base.before_first,
                     before_second_stdout=b'', load=load)


def stock_primary_load_models() -> list[HeldModel]:
    source = (b'include child.mk\nall: ; @printf \'%s\\n\' "$(PRIMARY) $(CHILD)"\n'
              b'child.mk: ; @printf \'%s\\n\' \'CHILD := loaded\' > $@\n')
    start = b'cat Makefile.sh >Makefile \n'
    mode_change = b'chmod a+x Makefile\n'
    dry_child = b'printf \'%s\\n\' \'CHILD := loaded\' > child.mk\n'
    dry_goal = b'printf \'%s\\n\' "suffix "\n'
    up_to_date = b"make: 'child.mk' is up to date.\n"
    touch_child = b'touch child.mk\n'
    worker = (SOURCES / 'stock-primary-cat.py').read_bytes()
    results = []
    for transport in ('pipe', 'fifo'):
        for tag, load_input in (('high', LOAD_HIGH), ('low', LOAD_LOW)):
            for optional, mode in ((False, 'n'), (False, 'q'), (False, 't'), (True, 'n'), (True, 't')):
                include_source = source.replace(b'include child.mk', b'-include child.mk') if optional else source
                replacement = include_source + b'PRIMARY := suffix\n'
                files = {'Makefile': include_source, 'Makefile.sh': replacement,
                         'stock-primary-cat.py': worker, 'load.input': load_input, 'seen': ANCHOR}
                times = {**dict.fromkeys(files, INPUT_TIME), 'Makefile.sh': INPUT_TIME + 1_000_000_000}
                updates = {'primary.started': b'ready\n', 'primary.release': b'yes\n',
                           'primary.done': b'done\n'}
                release_stdout, load_trace, stderr = start, b'proc\n', b''
                if mode == 'n':
                    updates['Makefile'] = replacement
                    if tag == 'low':
                        release_stdout += dry_child
                        stdout = release_stdout + mode_change + dry_child + up_to_date + dry_goal
                    else:
                        stdout = start + mode_change + dry_child * 2 + up_to_date + dry_goal
                elif mode == 'q':
                    stdout = start
                    stderr = b"make: *** Deleting file 'Makefile'\n"
                else:
                    updates.update({'Makefile': replacement, 'child.mk': b'', 'all': b''})
                    release_stdout += touch_child
                    load_trace = b''
                    stdout = release_stdout + b'touch Makefile\n' * 2 + up_to_date + b'touch all\n'
                expected = {'exit_code': int(mode == 'q'), 'stdout_template_hex': stdout.hex(),
                            'stderr_template_hex': stderr.hex()}
                args = ('--no-print-directory', '-j2', '-l2', f'--jobserver-style={transport}',
                        f'-{mode}', 'child.mk', 'all')
                name = 'load-stock-primary-optional-recipe' if optional else 'load-stock-primary-recipe'
                model = debug.Model(f'{name}-{mode}-{transport}-{tag}', files,
                                    args, {'GNUMAKEFLAGS': ''}, expected, ANCHOR,
                                    updates=updates, mtimes_ns=times)
                results.append(HeldModel(model, False, None, before_first_stdout=release_stdout,
                                         load=LoadProfile('normal', load_input, load_trace),
                                         stock_primary=True))
    return results


def optional_named_load_model(held: HeldModel, position: str, mode: str, high: bool) -> HeldModel:
    model = held.model
    lines = model.files['Makefile'].splitlines(keepends=True)
    for index, line in enumerate(lines):
        if line.startswith(b'include '):
            names = line.removeprefix(b'include ').split()
            lines[index] = b''.join((b'-include ' if name == b'named.gen' else b'include ')
                                    + name + b'\n' for name in names)
            break
    files = {**model.files, 'Makefile': b''.join(lines)}
    stdout = bytes.fromhex(model.expected['stdout_template_hex'])
    seen, updates, expected = model.seen, model.updates, model.expected
    if mode == 'q' and (position != 'middle' or not high):
        seen = seen.removesuffix(b'ordinary---\n')
        missing = b"make: *** No rule to make target 'named.gen'.  Stop.\n"
        expected = {**expected, 'exit_code': 2, 'stderr_template_hex': missing.hex()}
    if position == 'middle':
        if mode == 'n' and high:
            # The optional write ends the first pass before the simulated
            # right recipe can be run in ordinary mode on the next read.
            prefix = b'python3 worker.py right include\nprintf "tail\\n" >> seen\n'
            before_second = held.before_second + b'read-1\ntail-expanded\n'
            seen = before_second + b'right\ntail\nread-2\nordinary-loaded-loaded-rebuilt\n'
            stdout = prefix + stdout
            held = replace(held, before_second=before_second, before_second_stdout=prefix)
        elif mode == 'q' and high:
            # The queued right recipe returns a question verdict without
            # spawning, so only the already written optional source remains.
            seen = held.before_second + b'ordinary---\n'
            updates = {name: value for name, value in updates.items()
                       if not name.startswith('right.')}
            held = replace(held, before_second=None, before_second_stdout=None)
        elif mode in ('n', 't'):
            # An immediately simulated named recipe ends the pass before
            # the right recipe expands. It starts after the left has exited.
            second = BEFORE_FIRST + b'left\ntail-expanded\n'
            seen = seen.replace(held.before_second, second, 1)
            held = replace(held, overlap=False, before_first=BEFORE_FIRST, before_second=second)
            if mode == 't':
                prefix = held.before_first_stdout + b'touch left.gen\n'
                stdout = prefix + stdout.removeprefix(held.before_first_stdout)
                held = replace(held, before_second_stdout=prefix, load=replace(held.load, trace=b''))
    expected = {**expected, 'stdout_template_hex': stdout.hex()}
    model = replace(model, id=model.id.replace('load-parallel-', 'load-optional-parallel-', 1),
                    files=files, expected=expected, seen=seen, updates=updates)
    return replace(held, model=model)


def load_build_mode_models(available: list[HeldModel], markers: dict[str, bytes]) -> list[HeldModel]:
    left_markers = {name: data for name, data in markers.items() if name.startswith('left.')}
    dry_worker = b'python3 worker.py right success\n'
    touch_worker = b'touch right\ntouch right\n'
    high = LoadProfile('normal', LOAD_HIGH, b'proc\n')
    low = LoadProfile('normal', LOAD_LOW, b'proc\n')
    results = []
    for held in available:
        model = held.model
        if model.id.startswith('carried-optional-mode-') and model.id.endswith('-none'):
            shape, tail, mode, transport, _ = model.id.removeprefix('carried-optional-mode-').split('-')
            files = {**model.files, 'load.input': LOAD_HIGH}
            args = ('-l2', *model.args)
            ident = f'load-carried-mode-{shape}-{tail}-{mode}-{transport}'
            if shape == 'keep':
                queued = replace(model, id=ident + '-high', files=files, args=args,
                                 mtimes_ns=dict.fromkeys(files, INPUT_TIME))
                results.append(replace(held, model=queued, overlap=False, load=high))
                continue

            before = held.before_first + b'left\n'
            stdout = b'left\n'
            if mode == 'n':
                stdout += dry_worker
                if tail == 'tail':
                    stdout += b'printf "right-tail\\n" >> seen\n'
                stdout += b'printf "ordinary\\n" > "result"\n'
            if mode == 't':
                stdout += touch_worker + b'touch result\n'
            updates = {**left_markers, **({'right': b'', 'result': b''} if mode == 't' else {})}
            expected = {**model.expected, 'stdout_template_hex': stdout.hex(), 'stderr_template_hex': ''}
            queued = replace(model, id=ident + '-high', files=files, args=args, updates=updates,
                             expected=expected, seen=before if mode == 't' else before + b'ordinary\n',
                             mtimes_ns=dict.fromkeys(files, INPUT_TIME))
            results.append(HeldModel(queued, False, None, held.before_first, load=high))
            if tail == 'tail':
                files = {**model.files, 'load.input': LOAD_LOW}
                overlapping = replace(model, id=ident + '-low', files=files, args=args,
                                      mtimes_ns=dict.fromkeys(files, INPUT_TIME))
                results.append(replace(held, model=overlapping, load=low))
        elif model.id.startswith('load-optional-stop-'):
            # A goal with only secondary-expansion effects does not inherit
            # the queued optional recipe's question result.
            summary = b"make: Nothing to be done for 'all'.\n"
            modes = (('n', dry_worker + summary), ('q', b''), ('t', touch_worker + summary))
            for mode, stdout in modes:
                expected = {**model.expected, 'stdout_template_hex': (b'left\n' + stdout).hex()}
                ident = model.id.replace('load-optional-stop-', 'load-optional-build-mode-') + '-' + mode
                queued = replace(model, id=ident, args=('-' + mode, *model.args), expected=expected,
                                 updates={**left_markers, **({'right': b''} if mode == 't' else {})},
                                 seen=held.before_second)
                results.append(HeldModel(queued, False, None, held.before_first, load=high))
    return results


def load_goal_summary_models(worker: bytes, markers: dict[str, bytes]) -> list[HeldModel]:
    left = ('$(file >>seen,read-$(MAKE_RESTARTS))\n'
            'left: ; @$(file >>seen,expanded-left)python3 worker.py left success\n')
    right = '{target}: ; @{ignore}$(file >>seen,expanded-right)python3 worker.py right {action}\n'
    phony = '.PHONY: all\n'
    ordinary = right.format(target='all', ignore='', action='success') + phony
    build_file = right.format(target='right.gen', ignore='', action='include')
    ignored = right.format(target='all', ignore='-', action='fail') + phony
    failing = right.format(target='all', ignore='', action='fail') + phony
    nothing = b"make: Nothing to be done for 'all'.\n"
    current = b"make: 'right.gen' is up to date.\n"
    ignored_error = b'make: [Makefile:3: all] Error 4 (ignored)\n'
    recipe_error = b'make: *** [Makefile:3: all] Error 4\n'
    generated = {'right.gen': b'RIGHT := loaded\n'}
    cases = (
        ('deferred-phony', ordinary, 'all', False, (), 0, nothing, b'', {}),
        ('deferred-file', build_file, 'right.gen', False, (), 0, current, b'', generated),
        ('deferred-prerequisite', build_file + 'all: right.gen\n', 'all', False,
         (), 0, nothing, b'', generated),
        ('deferred-ignored', ignored, 'all', False, (), 0, b'right\n' + nothing, ignored_error, {}),
        ('deferred-failed', failing, 'all', False, (), 2, b'right\n', recipe_error, {}),
        ('deferred-failed-keep', failing, 'all', False, ('-k',), 2, b'right\n', recipe_error, {}),
        ('immediate-phony', ordinary, 'all', True, (), 0, b'', b'', {}),
        ('immediate-ignored', ignored, 'all', True, (), 0, b'right\n', ignored_error, {}),
    )
    expanded = BEFORE_FIRST + b'expanded-left\nexpanded-right\n'
    before_second = expanded + b'left\n'
    results = []
    for transport in ('pipe', 'fifo'):
        for name, source, goal, overlap, flags, status, stdout, stderr, updates in cases:
            load = LOAD_LOW if overlap else LOAD_HIGH
            files = {'Makefile': (left + source).encode(), 'worker.py': worker,
                     'seen': ANCHOR, 'load.input': load}
            args = ('-rR', '--no-print-directory', '-j2', '-l2',
                    f'--jobserver-style={transport}', *flags, 'left', goal)
            expected = {'exit_code': status, 'stdout_template_hex': stdout.hex(),
                        'stderr_template_hex': stderr.hex()}
            model = debug.Model(f'load-goal-{name}-{transport}', files, args, {'GNUMAKEFLAGS': ''},
                                expected, before_second + b'right\n', updates={**markers, **updates},
                                mtimes_ns=dict.fromkeys(files, INPUT_TIME))
            results.append(HeldModel(model, overlap, before_second, expanded, before_second_stdout=b'',
                                     load=LoadProfile('normal', load, b'proc\n')))
    return results


def held_primary_debug_model(held: HeldModel) -> HeldModel:
    model = held.model
    command = b'+@printf "# remade\\n" >> "$@"'
    makefile = model.files['Makefile']
    if makefile.count(command) != 1 or held.before_second is None:
        raise h.HarnessError('invalid named primary debug fixture')
    makefile = makefile.replace(command, b'+@python3 worker.py primary append-primary')
    files = {**model.files, 'Makefile': makefile}
    updates = {**model.updates, 'Makefile': makefile + b'# remade\n',
               'primary.started': b'started\n', 'primary.release': b'yes\n',
               'primary.done': b'done\n'}
    prefix = held.before_first
    first = prefix + b'primary\n'
    model = replace(model, id=model.id.replace('forced-stale', 'held-forced-stale'),
                    files=files, updates=updates, seen=model.seen.replace(prefix, first, 1))
    return replace(held, model=model, before_primary=prefix, before_first=first,
                   before_second=held.before_second.replace(prefix, first, 1))


def shuffled_remake_models(held_models: list[HeldModel]) -> list[HeldModel]:
    by_id = {held.model.id: held for held in held_models}
    chosen = (
        'named-include-file-prerequisite-required-first-n-pipe',
        'named-include-file-prerequisite-required-last-q-fifo',
        'named-include-file-prerequisite-optional-middle-n-pipe',
        'named-include-file-prerequisite-optional-middle-t-fifo',
        'primary-named-group-primary-first-stale-n-fifo',
        'primary-named-group-primary-first-missing-peer-q-pipe',
        'primary-named-wait-shared-left-right-n-pipe-older',
        'primary-named-wait-shared-left-right-t-fifo-older',
    )
    results = []
    for name in chosen:
        held = by_id[name]
        model = held.model
        lines = bytes.fromhex(model.expected['stdout_template_hex']).splitlines(keepends=True)
        # For these seeds the two ordinary goal reports reverse. The remake
        # output before those reports and both held release boundaries agree.
        if lines:
            lines[-2:] = reversed(lines[-2:])
        expected = {**model.expected, 'stdout_template_hex': b''.join(lines).hex()}
        for shuffle in ('reverse', '1'):
            variant = replace(model, id=f'{name}-shuffle-{shuffle}',
                              args=(*model.args, f'--shuffle={shuffle}'), expected=expected)
            results.append(replace(held, model=variant))
    return results


def hold_debug_tail(model: debug.Model) -> debug.Model:
    source = model.files['Makefile']
    command = b'printf "TAIL := loaded\\n" > "$@"'
    if source.count(command) != 1:
        raise h.HarnessError('tail release requires one tail recipe')
    files = {**model.files, 'Makefile': source.replace(command, b'python3 tail-witness.py'),
             'tail-witness.py': (SOURCES / 'tail-witness.py').read_bytes()}
    return replace(model, files=files, updates={**model.updates, 'tail.release': b'yes\n'},
                   mtimes_ns={**model.mtimes_ns, 'tail-witness.py': model.mtimes_ns['Makefile']})


def named_load_debug_witness(model: debug.Model) -> debug.Model:
    original = b'named.gen: ; @printf "VALUE := rebuilt\\n" > named.gen'
    source = model.files['Makefile']
    if original not in source:
        raise h.HarnessError('invalid named load recipe witness')
    files = {**model.files, 'Makefile': source.replace(
        original, b'named.gen: ; @python3 worker.py named named-value')}
    updates = {**model.updates, 'named.started': b'started\n',
               'named.release': b'yes\n', 'named.done': b'done\n'}
    return replace(model, files=files, updates=updates)


def directory_named_load_model(held: HeldModel) -> HeldModel:
    model = held.model
    leaf, path = b'named.gen', b'cfg/named.gen'
    files = {**model.files, 'cfg/.keep': b''}
    files['Makefile'] = files['Makefile'].replace(leaf, path)
    if held.named_recipe is not None:
        files['worker.py'] = files['worker.py'].replace(leaf, path)
    updates = {name.replace(leaf.decode(), path.decode()): content for name, content in model.updates.items()}
    expected = {**model.expected}
    for stream in ('stdout_template_hex', 'stderr_template_hex'):
        expected[stream] = bytes.fromhex(expected[stream]).replace(leaf, path).hex()
    args = tuple(arg.replace(leaf.decode(), path.decode()) for arg in model.args)
    model = replace(model, id='directory-' + model.id, files=files, args=args, updates=updates,
                    expected=expected, mtimes_ns={**model.mtimes_ns, 'cfg/.keep': INPUT_TIME})
    return replace(held, model=model,
                   before_first_stdout=held.before_first_stdout.replace(leaf, path),
                   before_second_stdout=(held.before_second_stdout.replace(leaf, path)
                                         if held.before_second_stdout is not None else None),
                   second_stdout_barrier=(held.second_stdout_barrier.replace(leaf, path)
                                          if held.second_stdout_barrier is not None else None),
                   named_recipe=path if held.named_recipe is not None else None)


def builtin_load_model(held: HeldModel, *, explicit: bool = False) -> HeldModel:
    model = held.model
    files = model.files
    if explicit:
        files = {**files, 'Makefile': files['Makefile'] + b'Makefile: ;\n'}
    primary_visit = b" Considering target file 'Makefile'.\n"
    stock_visit = primary_visit + b'Directory . cache invalidated (count 1 != command 2)\n'

    def builtin_stdout(stdout: bytes) -> bytes:
        stdout = stdout.replace(b' -rR ', b' ')
        return stdout if explicit else stdout.replace(primary_visit, stock_visit)

    expected = {**model.expected}
    stdout = bytes.fromhex(expected['stdout_template_hex'])
    expected['stdout_template_hex'] = builtin_stdout(stdout).hex()
    prefix = 'explicit-builtin-' if explicit else 'builtin-'
    model = replace(model, id=prefix + model.id, files=files, expected=expected,
                    args=tuple(arg for arg in model.args if arg != '-rR'))
    second = held.before_second_stdout
    return replace(held, model=model,
                   before_first_stdout=builtin_stdout(held.before_first_stdout),
                   before_second_stdout=builtin_stdout(second) if second is not None else None)


def remake_debug_models(held_models: list[HeldModel]) -> list[HeldModel]:
    data = tomllib.loads((SOURCES / 'debug-models.toml').read_text())
    if data.get('schema_version') != 1:
        raise h.HarnessError('unsupported remake debug model schema')
    by_id = {held.model.id: held for held in held_models}
    for mode in 'nqt':
        base = f'later-primary-named-mode-forced-stale-{mode}-pipe-3-slots'
        held = held_primary_debug_model(by_id[base])
        by_id[held.model.id] = held
    selected = []
    seen = set()
    has_load_profiles = any(held.load is not None for held in held_models)
    for row in data['profiles']:
        name, flag = row['case'], row['flag']
        fifo = row.get('fifo', False)
        trace_transports = row.get('second_trace_transports', [])
        tail_barrier = row.get('tail_stdout_barrier')
        second_barrier = row.get('second_stdout_barrier')
        hold_named = row.get('hold_named', False)
        requires_load = row.get('requires_load', False)
        if requires_load is True and not has_load_profiles:
            continue
        held = by_id.get(name)
        if (held is None or (name, flag) in seen or not isinstance(requires_load, bool)
                or (held.load is not None) != requires_load
                or flag not in ('m', 'm,v') or not isinstance(fifo, bool)
                or not isinstance(hold_named, bool)
                or (hold_named and (flag != 'm,v' or not requires_load or trace_transports))
                or (fifo and flag != 'm,v')
                or not isinstance(trace_transports, list)
                or any(transport not in ('pipe', 'fifo') for transport in trace_transports)
                or len(trace_transports) != len(set(trace_transports))
                or ('fifo' in trace_transports and not fifo)
                or (trace_transports and flag != 'm,v')
                or (trace_transports and second_barrier is not None)
                or (tail_barrier is not None and
                    (not requires_load or set(trace_transports) != {'pipe', 'fifo'}))
                or (flag == 'm,v' and '--jobserver-style=pipe' not in held.model.args)):
            raise h.HarnessError('invalid remake debug model')
        seen.add((name, flag))
        stdout = row['stdout'].encode()
        prefixes = row['release_prefixes']
        primary = held.before_primary is not None
        count = 1 + int(held.before_second is not None) + int(primary)
        if (len(prefixes) != count or prefixes != sorted(prefixes)
                or not 0 < prefixes[0] <= prefixes[-1] <= len(stdout)):
            raise h.HarnessError('invalid remake debug release prefixes')
        if primary and any(not stdout[:end].endswith(barrier)
                           for end, barrier in zip(prefixes, PRIMARY_BARRIERS, strict=True)):
            raise h.HarnessError('invalid named primary debug release barrier')
        first = prefixes[int(primary)]
        second = prefixes[-1] if held.before_second is not None else None
        second_trace = stdout[first:second] if second is not None else b''
        if trace_transports and (not second_trace or b'@BIN@' in second_trace
                                 or b'Using jobserver controller ' in second_trace):
            raise h.HarnessError('invalid second release trace barrier')
        if tail_barrier is not None:
            if (not isinstance(tail_barrier, str) or not tail_barrier
                    or tail_barrier.encode() not in second_trace):
                raise h.HarnessError('invalid tail release trace barrier')
            tail_barrier = tail_barrier.encode()
        args = (*held.model.args[:-1], f'--debug={flag}', held.model.args[-1])
        model = replace(held.model, id=f'remake-debug-{name}-{flag.replace(",", "")}', args=args,
                        expected={**held.model.expected, 'stdout_template_hex': stdout.hex()})
        if tail_barrier is not None:
            model = hold_debug_tail(model)
        if hold_named:
            if primary or held.release_order != WORKERS:
                raise h.HarnessError('invalid named load recipe witness')
            if second is not None and second_barrier is None:
                second_barrier = NAMED_LOAD_RIGHT_BARRIER.decode()
            model = named_load_debug_witness(model)
            held = replace(held, named_recipe=b'named.gen')
        if second_barrier is not None:
            if (not isinstance(second_barrier, str) or not second_barrier or second is None
                    or not stdout[:second].endswith(second_barrier.encode())):
                raise h.HarnessError('invalid second release trace barrier')
            held = replace(held, second_stdout_barrier=second_barrier.encode())
        selected.append(replace(held, model=model, before_first_stdout=stdout[:first],
                                before_second_stdout=stdout[:second] if second is not None else None,
                                before_primary_stdout=stdout[:prefixes[0]] if primary else b'',
                                second_stdout_barrier=(second_trace if 'pipe' in trace_transports
                                                       else held.second_stdout_barrier),
                                tail_stdout_barrier=tail_barrier))
        if fifo:
            # Keep the same expected trace around the separately verified live path.
            if '-pipe' not in name or b'Using jobserver controller 3,4\n' not in stdout[:first]:
                raise h.HarnessError('FIFO debug model requires a pipe controller trace')
            fifo_stdout = stdout.replace(b'--jobserver-style=pipe', b'--jobserver-style=fifo')
            fifo_args = tuple('--jobserver-style=fifo' if arg == '--jobserver-style=pipe' else arg
                              for arg in args)
            fifo_model = replace(model, id=model.id.replace('-pipe', '-fifo'), args=fifo_args,
                                 expected={**model.expected, 'stdout_template_hex': fifo_stdout.hex()})
            selected.append(replace(held, model=fifo_model, fifo_controller=True,
                                    before_first_stdout=fifo_stdout[:first],
                                    before_second_stdout=fifo_stdout[:second] if second is not None else None,
                                    before_primary_stdout=fifo_stdout[:prefixes[0]] if primary else b'',
                                    second_stdout_barrier=(second_trace if 'fifo' in trace_transports
                                                           else held.second_stdout_barrier),
                                    tail_stdout_barrier=tail_barrier))
    variants = []
    for held in selected:
        if held.model.id.startswith(('remake-debug-load-parallel-named-mode-',
                                     'remake-debug-load-optional-parallel-named-mode-')):
            variants.extend((directory_named_load_model(held), builtin_load_model(held, explicit=True),
                             builtin_load_model(held)))
    return selected + variants


def active_workers(parent: int, proc: Path,
                   allowed: tuple[str, ...] = WORKERS) -> dict[str, tuple[int, int]]:
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
        if name not in allowed or name in workers or owner != parent or started_at <= 0:
            raise h.HarnessError('invalid held remake worker identity')
        workers[name] = (pid, started_at)
    return workers


def poll_wait(proc: Path, channel: str = '') -> tuple[int | None, bool]:
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
        if number == 270 and channel.split('.', 1)[0] == 'poll_schedule_timeout':
            # GNU first waits with a timeout while load-delayed work exists.
            # Once it starts that work, its next descriptor wait can be
            # indefinite. The sleeping task and poll channel qualify both.
            state = (proc / 'stat').read_bytes().rpartition(b') ')[2]
            blocking = (state.startswith(b'S ') and int(fields[1], 0) > 0
                        and any(int(value, 0) for value in fields[2:5]))
        return number, blocking
    except (OSError, ValueError):
        return None, False


def execute(case: h.Case, runtime: Path, binary: Path,
            wait_samples: set[tuple[str, int | None]] | None = None, *,
            load: LoadProfile | None = None, load_shim: Path | None = None,
            wait_stdout: Callable[[Path, bytes], bytes] | None = None,
            release_order: tuple[str, str] = WORKERS, hold_primary: bool = False,
            second_stdout_barrier: bytes | None = None,
            tail_stdout_barrier: bytes | None = None, named_recipe: bytes | None = None):
    if sorted(release_order) != list(WORKERS):
        raise h.HarnessError('held release order must name both workers')
    order = ('primary', *release_order) if hold_primary else release_order
    named_barrier = NAMED_LOAD_BARRIER.replace(b'named.gen', named_recipe) if named_recipe is not None else None
    allowed = (*order, 'named') if named_barrier is not None else order
    work = runtime / 'work'
    trace = runtime / 'load.trace'
    if load is not None:
        if load_shim is None:
            raise h.HarnessError('load profiles require a preload shim')
        trace.unlink(missing_ok=True)
        case = replace(case, environment={**case.environment, 'LD_PRELOAD': str(load_shim),
                                          'RMAKE_TEST_LOAD': load.mode, 'RMAKE_LOAD_TRACE': str(trace),
                                          'RMAKE_LOAD_AVERAGE': '100'})
    releases = []
    released = set()
    previous = None

    def observe(parent: int, stdout: bytes) -> None:
        nonlocal previous
        phase = len(releases)
        if phase == len(order):
            return
        if hold_primary and not stdout.endswith(PRIMARY_BARRIERS[phase]):
            return
        if (phase == len(order) - 1 and tail_stdout_barrier is not None
                and tail_stdout_barrier in stdout):
            marker = work / 'tail.release'
            if not marker.exists():
                marker.write_bytes(b'yes\n')
        proc = Path('/proc', str(parent))
        try:
            if named_barrier is not None and phase == 1 and not (work / 'named.release').exists():
                # Let both active roots report their pending state before the
                # short named recipe can finish and advance the remake pass.
                if named_barrier in stdout and 'named' in active_workers(parent, proc, allowed):
                    if (work / 'named.started').read_bytes() != b'started\n':
                        raise h.HarnessError('unexpected held named recipe marker')
                    temporary = work / 'named.release.tmp'
                    temporary.write_bytes(b'yes\n')
                    temporary.chmod(0o644)
                    temporary.rename(work / 'named.release')
                return
            if (second_stdout_barrier is not None and phase == len(order) - 1
                    and not stdout.endswith(second_stdout_barrier)):
                return
            channel = (proc / 'wchan').read_text().strip()
            number, blocked = (None, True) if channel in WAIT_CHANNELS else poll_wait(proc, channel)
            if wait_samples is not None:
                wait_samples.add((channel, number))
            if not blocked:
                return
            workers = active_workers(parent, proc, allowed)
            if 'named' in workers:
                return
            if not workers or released.intersection(workers):
                return
            if hold_primary and (set(workers) != set(order[phase:])
                                 or any((work / f'{name}.done').read_bytes() != b'done\n'
                                        for name in order[:phase])):
                return
            for name in workers:
                if (work / f'{name}.started').read_bytes() != b'started\n':
                    raise h.HarnessError('unexpected held remake worker marker')
                if (work / f'{name}.done').exists() or (work / f'{name}.release').exists():
                    raise h.HarnessError('remake worker escaped its release barrier')
            seen = (work / 'seen').read_bytes()
            load_bytes = (work / 'load.input').read_bytes() if load is not None else b''
            load_trace = trace.read_bytes() if load is not None and trace.exists() else b''
        except (FileNotFoundError, ProcessLookupError):
            return
        if (load is not None and load.mode == 'drop-on-token-wait' and not released
                and load_trace and b'expanded-tail\n' in seen and 'right' not in workers):
            return
        # Let a serial baseline progress when it demonstrably waits on only one
        # child. The oracle and candidate still have to match the expected set.
        signature = (sorted(workers.items()), channel, number, seen, stdout, load_bytes, load_trace)
        if signature != previous:
            previous = signature
            return
        release_stdout = wait_stdout(proc, stdout) if wait_stdout is not None else stdout
        name = next(name for name in order if name in workers)
        temporary = work / f'{name}.release.tmp'
        temporary.write_bytes(b'yes\n')
        temporary.chmod(0o644)
        temporary.rename(work / f'{name}.release')
        releases.append({'name': name, 'parent': parent, 'workers': workers,
                         'wchan': channel, 'poll_syscall': number,
                         'seen_hex': seen.hex(), 'stdout_hex': release_stdout.hex()})
        if wait_stdout is not None:
            releases[-1]['stdout_raw_hex'] = stdout.hex()
        if load is not None:
            releases[-1].update(load_hex=load_bytes.hex(), load_trace_hex=load_trace.hex())
        released.add(name)

    return h.execute(case, runtime, binary, observe=observe), releases


def execute_stock_primary(case: h.Case, runtime: Path, binary: Path, held: HeldModel,
                          load_shim: Path) -> tuple[h.Observation, list[dict]]:
    h.reset_workspace(case, runtime, binary)
    work = runtime / 'work'
    script = work / 'stock-primary-cat.py'
    script.chmod(0o755)
    wrapper = runtime / 'bin' / 'cat'
    wrapper.unlink(missing_ok=True)
    wrapper.symlink_to(script)
    trace = runtime / 'load.trace'
    trace.unlink(missing_ok=True)
    environment = h.clean_environment(runtime, {
        **case.environment, 'LD_PRELOAD': str(load_shim), 'RMAKE_TEST_LOAD': 'normal',
        'RMAKE_LOAD_TRACE': str(trace), 'RMAKE_LOAD_AVERAGE': '100',
    })
    releases = []

    def observe(parent: int, stdout: bytes) -> None:
        if releases:
            return
        proc = Path('/proc', str(parent))
        try:
            if (work / 'primary.started').read_bytes() != b'ready\n':
                return
            channel = (proc / 'wchan').read_text().strip()
            if channel not in WAIT_CHANNELS and not poll_wait(proc, channel)[1]:
                return
            children = (proc / 'task' / str(parent) / 'children').read_bytes().split()
            for child in map(int, children):
                child_proc = Path('/proc', str(child))
                shell_argv = child_proc.joinpath('cmdline').read_bytes().split(b'\0')
                if shell_argv != [b'/bin/sh', b'-c', b'cat Makefile.sh >Makefile ', b'']:
                    continue
                shell_owner, shell_started = proc_identity(child)
                if shell_owner != parent or shell_started <= 0:
                    raise h.HarnessError('invalid stock primary shell identity')
                grandchildren = child_proc.joinpath('task', str(child), 'children').read_bytes().split()
                if len(grandchildren) != 1:
                    continue
                pid = int(grandchildren[0])
                command = Path('/proc', str(pid), 'cmdline').read_bytes().split(b'\0')
                if command != [b'/usr/bin/python3', os.fsencode(wrapper), b'Makefile.sh', b'']:
                    continue
                owner, started = proc_identity(pid)
                if owner != child or started <= 0 or (work / 'primary.done').exists():
                    raise h.HarnessError('invalid stock primary child identity')
                load_trace = trace.read_bytes() if trace.exists() else b''
                if (held.load is not None and held.load.at_release == LOAD_LOW
                        and '-n' in case.args and load_trace and stdout != held.before_first_stdout):
                    return
                temporary = work / 'primary.release.tmp'
                temporary.write_bytes(b'yes\n')
                temporary.chmod(0o644)
                temporary.rename(work / 'primary.release')
                releases.append({'name': 'primary', 'parent': parent, 'shell': child, 'pid': pid,
                                 'started': started,
                                 'stdout_hex': stdout.hex(), 'load_trace_hex': load_trace.hex(),
                                 'load_hex': (work / 'load.input').read_bytes().hex(), 'wchan': channel})
                return
        except (FileNotFoundError, ProcessLookupError):
            return

    try:
        out, err, status, limit, leftovers = h.capture(
            [str(runtime / 'bin' / 'make'), *case.args], work, environment, case.timeout,
            observe=observe)
    finally:
        wrapper.unlink(missing_ok=True)
    filesystem = h.snapshot({name: runtime / name for name in ('work', 'home', 'tmp')})
    return h.Observation(out, err, status, limit, filesystem, leftovers), releases


def bind_fifo_controller(observation: h.Observation, releases: list[dict], runtime: Path):
    if not releases:
        return observation, releases, ['missing live FIFO controller identity']
    prefix = b'Using jobserver controller '
    path = runtime / 'tmp' / f"GMfifo{releases[0]['parent']}"
    controller = prefix + b'fifo:' + os.fsencode(path) + b'\n'
    errors = []

    def bind(stdout: bytes, phase: str) -> bytes:
        lines = [line for line in stdout.splitlines(keepends=True) if line.startswith(prefix)]
        if not lines or any(line != controller for line in lines):
            errors.append(f'{phase} FIFO controller identity')
            return stdout
        return stdout.replace(controller, prefix + b'3,4\n')

    stdout = bind(observation.stdout, 'final')
    prefixes = [dict(release, stdout_hex=bind(bytes.fromhex(release['stdout_hex']),
                                             release['name']).hex()) for release in releases]
    return replace(observation, stdout=stdout), prefixes, errors


def release_errors(held: HeldModel, releases: list[dict], runtime: Path) -> list[str]:
    if held.stock_primary:
        if len(releases) != 1:
            return ['live stock primary release']
        (release,) = releases
        errors = []
        if release['stdout_hex'] != held.before_first_stdout.hex():
            errors.append('stock primary release stdout')
        if (release['load_hex'] != held.load.at_release.hex()
                or release['load_trace_hex'] != held.load.trace.hex()):
            errors.append('stock primary release load observation')
        return errors
    order = list(held.release_order[:1] if held.before_second is None else held.release_order)
    wanted = ['primary', *order] if held.before_primary is not None else order
    if [release['name'] for release in releases] != wanted:
        return ['held release sequence']
    errors = []
    if held.before_primary is not None:
        primary, *releases = releases
        first = releases[0]
        if set(primary['workers']) != {'primary', *WORKERS}:
            errors.append('active workers at primary release')
        if primary['parent'] != first['parent'] or any(
            primary['workers'].get(name) != first['workers'].get(name) for name in WORKERS
        ):
            errors.append('worker identities after primary release')
        if primary['seen_hex'] != held.before_primary.hex():
            errors.append('file effects at primary release')
        if primary['stdout_hex'] != debug.render(held.before_primary_stdout, runtime).hex():
            errors.append('stdout at primary release')
    if held.load is not None and any(
        release.get('load_hex') != held.load.at_release.hex()
        or release.get('load_trace_hex') != (
            held.load.second_trace if index == 1 and held.load.second_trace is not None
            else held.load.trace
        ).hex()
        for index, release in enumerate(releases)
    ):
        errors.append('load observation at release')
    first = releases[0]
    expected_first = list(WORKERS) if held.overlap else order[:1]
    if sorted(first['workers']) != expected_first:
        errors.append('active workers at first release')
    if first['seen_hex'] != held.before_first.hex():
        errors.append('file effects at first release')
    if first['stdout_hex'] != debug.render(held.before_first_stdout, runtime).hex():
        errors.append('stdout at first release')
    if held.before_second is None:
        return errors
    second = releases[1]
    survivor = order[1]
    if list(second['workers']) != [survivor]:
        errors.append('active workers at second release')
    if first['parent'] != second['parent']:
        errors.append('parent identity at release')
    if (held.overlap and survivor in first['workers']
            and first['workers'][survivor] != second['workers'].get(survivor)):
        errors.append('surviving worker identity across releases')
    if second['seen_hex'] != held.before_second.hex():
        errors.append('file effects at second release')
    stdout_before_second = held.before_second_stdout
    if stdout_before_second is None:
        stdout_before_second = order[0].encode() + b'\n' if held.model.expected['stdout_template_hex'] else b''
    if second['stdout_hex'] != debug.render(stdout_before_second, runtime).hex():
        errors.append('stdout at second release')
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=reference_oracle.PREFIX / 'bin' / 'make')
    parser.add_argument('--oracle-profile', type=Path, default=reference_oracle.PROFILE)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--self-test', action='store_true')
    mode.add_argument('--candidate', type=Path)
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument('--duplicate-reads', action='store_true')
    selection.add_argument('--remake-debug', action='store_true',
                           help='select only the remake-phase debug profiles')
    parser.add_argument('--case', action='append', default=[])
    load_options = parser.add_mutually_exclusive_group()
    load_options.add_argument('--include-load', action='store_true',
                              help='add load profiles and compile their tracked preload shim')
    load_options.add_argument('--load-shim', type=Path,
                              help='add load profiles using an existing preload shim')
    reports = parser.add_mutually_exclusive_group()
    reports.add_argument('--report', type=Path)
    reports.add_argument('--report-dir', type=Path)
    args = parser.parse_args()
    try:
        profile = reference_oracle.verify(args.oracle, args.oracle_profile)
        load_shim = args.load_shim.resolve(strict=True) if args.load_shim is not None else None
        if load_shim is not None and not load_shim.is_file():
            raise h.HarnessError('load shim must be a file')
        available = models(duplicate_reads=args.duplicate_reads,
                           include_load=args.include_load or load_shim is not None)
        if not args.duplicate_reads:
            debug_profiles = remake_debug_models(available)
            available = debug_profiles if args.remake_debug else available + debug_profiles
        unknown = set(args.case) - {held.model.id for held in available}
        if unknown:
            raise h.HarnessError('unknown remake cases: ' + ', '.join(sorted(unknown)))
        selected = [held for held in available if not args.case or held.model.id in args.case]
        executables = [args.oracle, args.oracle]
        if args.candidate is not None:
            executables.append(args.candidate)
        records = []
        load_build = None
        with tempfile.TemporaryDirectory(prefix='rmake-remake-parallel-') as scratch:
            root = Path(scratch)
            runtime = root / 'runtime'
            runtime.mkdir(mode=0o755)
            if load_shim is None and any(held.load is not None for held in selected):
                load_shim, load_build = compile_fixture(LOAD_SHIM_SOURCE, root)
            for held in selected:
                model = held.model
                case, filesystem = debug.prepare(model, root / model.id, runtime)
                samples = [set() for _ in executables]
                if held.stock_primary:
                    assert load_shim is not None
                    filesystem['work/stock-primary-cat.py']['mode'] = 0o755
                    if '-q' in case.args:
                        filesystem.pop('work/Makefile')
                    observations, releases = zip(*(
                        execute_stock_primary(case, runtime, binary, held, load_shim)
                        for binary in executables
                    ))
                else:
                    observations, releases = zip(*(
                        execute(case, runtime, binary, waits, load=held.load, load_shim=load_shim,
                                release_order=held.release_order, hold_primary=held.before_primary is not None,
                                second_stdout_barrier=held.second_stdout_barrier,
                                tail_stdout_barrier=held.tail_stdout_barrier, named_recipe=held.named_recipe)
                        for binary, waits in zip(executables, samples, strict=True)
                    ))
                raw_observations, raw_releases = observations, releases
                fifo_errors = [[] for _ in executables]
                if held.fifo_controller:
                    observations, releases, fifo_errors = zip(*(
                        bind_fifo_controller(observed, release, runtime)
                        for observed, release in zip(observations, releases, strict=True)
                    ))
                record = debug.assess(model, case, runtime, filesystem, list(observations))
                errors = [release_errors(held, release, runtime) + identity_errors
                          for release, identity_errors in zip(releases, fifo_errors, strict=True)]
                if held.fifo_controller:
                    record.update(raw_stdout_hex=[observed.stdout.hex() for observed in raw_observations],
                                  raw_releases=raw_releases)
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
        if load_build is not None:
            report['load_shim_build'] = load_build
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
