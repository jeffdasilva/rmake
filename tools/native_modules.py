#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Bounded original native-module differential tests; never a GNU runtime fallback.

A C compiler builds each module once per case in disposable input directories.
GNU/GNU/candidate then receive byte-identical reset inputs and executable aliases.
No ledger credit or safety-exception approval is assigned by this separate gate.
"""
from __future__ import annotations

import argparse
import base64
from dataclasses import dataclass, field, replace
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import tempfile
import tomllib
from typing import Any

import conformance as h
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / 'tests/native'
INPUT_TIME_NS = 1_600_000_000_000_000_000


@dataclass(frozen=True)
class Module:
    source: str = 'api.c'
    filename: str = 'probe_api.so'
    defines: tuple[str, ...] = ()


@dataclass(frozen=True)
class Model:
    id: str
    files: dict[str, bytes]
    expected: dict[str, Any]
    seen: bytes
    modules: tuple[Module, ...] = (Module(),)
    args: tuple[str, ...] = ('-rR', '--no-print-directory')
    environment: dict[str, str] = field(default_factory=lambda: {'GNUMAKEFLAGS': ''})
    mtimes_ns: dict[str, int] = field(default_factory=dict)
    symlinks: dict[str, str] = field(default_factory=dict)
    output: str = 'separate'


def expression_model(ident: str, expression: bytes, result: bytes, *,
                     preamble: bytes = b'', loads: bytes = b'load ./probe_api.so\n',
                     args: tuple[str, ...] = (), environment: dict[str, str] | None = None,
                     loaded: bytes = b'probe_api.so', seen: bytes = b'anchor\nbefore\n',
                     error: bytes | None = None, error_line: int | None = None,
                     modules: tuple[Module, ...] = (Module(),),
                     extra: dict[str, bytes] | None = None,
                     stderr: bytes | None = None, exit_code: int | None = None) -> Model:
    head = preamble + loads + b'VALUE = original\n$(file >>seen,before)\n'
    body = head + b'$(info RESULT=[' + expression + b'])\n$(info LOADED=[$(.LOADED)])\n.PHONY: all\nall:;@:\n'
    line = error_line if error_line is not None else head.count(b'\n') + 1
    diagnostic = (b'Makefile:' + str(line).encode() + b': *** ' + error + b'.  Stop.\n') if error else b''
    stdout = b'' if error else b'RESULT=[' + result + b']\nLOADED=[' + loaded + b']\n'
    return Model('native-' + ident, {'Makefile': body, 'seen': b'anchor\n', **(extra or {})},
                 {'exit_code': (2 if error else 0) if exit_code is None else exit_code, 'stdout_hex': stdout.hex(),
                  'stderr_hex': (diagnostic if stderr is None else stderr).hex()},
                 seen, modules, ('-rR', '--no-print-directory', *args),
                 {'GNUMAKEFLAGS': '', **(environment or {})})


def models() -> list[Model]:
    """Expected bytes and effect witnesses are authored independently of execution."""
    result = []
    def add(*args: Any, **kwargs: Any) -> None:
        result.append(expression_model(*args, **kwargs))
    for ident, expression, expected in (
        ('arguments', b'$(probe_args one,two,three)', b'argc=3[one][two][three]'),
        ('maximal-tail', b'$(probe_pair one,two,three)', b'argc=2[one][two,three]'),
        ('empty-argument', b'$(probe_args )', b'argc=1[]'),
        ('literal-variable-name', b'$(probe_args)', b''),
        ('empty-pair', b'$(probe_pair ,)', b'argc=2[][]'),
        ('lazy-native', b'$(probe_raw $(file >>seen,forbidden),second,third)', b'argc=2[$(file >>seen,forbidden)][second,third]'),
        ('expand-native', b'$(probe_expand $$(VALUE))', b'original'),
        ('expand-local', b'$(foreach VALUE,local,$(probe_expand $$(VALUE)))', b'local'),
        ('eval-native', b'$(probe_eval VALUE := updated)', b'updated'),
        ('where', b'$(probe_where )', b'Makefile:1'),
        ('allocation-free', b'$(probe_free )', b'released'),
        ('null-return', b'before$(probe_null )after', b'beforeafter'),
        ('call-native', b'$(call probe_args,one,two)', b'argc=2[one][two]'),
    ):
        add(ident, expression, expected)
    add('call-lazy', b'$(call probe_raw,$(file >>seen,argument)one,two)', b'argc=2[one][two]', seen=b'anchor\nbefore\nargument\n')
    add('registered-during-expansion', b'$(probe_install )$(created hello)', b'installedargc=1[hello]')
    add('function-over-variable', b'$(probe_args)|$(probe_args x)', b'variable|argc=1[x]', preamble=b'probe_args := variable\n')
    arity = b"insufficient number of arguments (1) to function 'probe_pair'"
    add('arity-after-expansion', b'$(probe_pair $(file >>seen,argument))', b'', seen=b'anchor\nbefore\nargument\n', error=arity)
    add('call-arity-after-arguments', b'$(call probe_pair,$(file >>seen,argument)one)', b'', seen=b'anchor\nbefore\nargument\n', error=arity)
    add('max-one-comma-tail', b'$(probe_expand one,two,three)', b'one,two,three')
    add('loaded-command-origin', b'$(probe_args x)', b'argc=1[x]', args=('.LOADED=prior',), loaded=b'prior')
    add('loaded-file-append', b'$(probe_args x)', b'argc=1[x]', preamble=b'.LOADED = prior\n', loaded=b'prior probe_api.so')
    add('loaded-append-after-undefine', b'$(origin .LOADED)|$(flavor .LOADED)|$(value .LOADED)',
        b'file|recursive|probe_api.so', preamble=b'undefine .LOADED\n')
    add('eval-crlf-buffer', b'$(probe_eval $(file <eval.txt))', b'one\r',
        extra={'eval.txt': b'VALUE := one\r\n# sentinel\n'})
    add('eval-define-body-location', b'$(probe_eval $(file <eval.txt))', b'one',
        stderr=b'Makefile:4: native-define-body\n',
        extra={'eval.txt': b'define VALUE :=\n$(warning native-define-body)one\nendef\n'})
    add('loaded-repeated-setup-location', b'$(probe_where )', b'Makefile:1', loads=b'load ./probe_api.so\nload ./probe_api.so\n')
    add('failed-required-setup', b'', b'', environment={'PROBE_SETUP_ZERO': '1'}, error=b'probe_api.so: failed to load', error_line=1, seen=b'anchor\n')
    for suffix, loads in (('registration', b'-load ./probe_api.so\n'), ('then-required', b'-load ./probe_api.so\nload ./probe_api.so\n')):
        add('failed-optional-setup-' + suffix, b'$(probe_args x)', b'argc=1[x]', loads=loads, environment={'PROBE_SETUP_ZERO': '1'}, loaded=b'')
    # The original 25 safe research inputs above are retained; no quarantined
    # built-in replacement callback is present in the source or this campaign.
    add('explicit-setup', b'$(probe_where )', b'Makefile:1', loads=b'load probe_api.so(alternate_setup)\n')
    add('nul-result', b'head$(probe_nul )tail', b'headoktail')
    add('raw-byte-result', b'$(probe_args raw\xff,tail)', b'argc=2[raw\xff][tail]')
    add('brace-reference', b'${probe_args one,two}', b'argc=2[one][two]')
    add('expand-reentrant-native', b'$(probe_expand $$(EXPR))', b'argc=2[one][two]', preamble=b'EXPR = $(probe_args one,two)\n')
    add('expand-call-local', b'$(call FN,slot)', b'slot', preamble=b'FN = $(probe_expand $$(1))\n')
    add('raw-arity-skips-effects', b'$(probe_raw_pair $(file >>seen,forbidden))', b'', error=b"insufficient number of arguments (1) to function 'probe_raw_pair'")
    add('eval-guard-success', b'$(probe_eval_guard VALUE := updated)', b'updated', seen=b'anchor\nbefore\neval-returned\n')
    add('expand-guard-success', b'$(probe_expand_guard $$(VALUE))', b'original', seen=b'anchor\nbefore\nexpand-returned\n')
    add('eval-include', b'$(probe_eval_guard include child.mk)', b'included', extra={'child.mk': b'VALUE := included\n$(file >>seen,include)\n'}, seen=b'anchor\nbefore\ninclude\neval-returned\n')
    add('eval-fatal-stop', b'$(probe_eval_guard $$(error native-stop))', b'', error=b'native-stop')
    add('expand-fatal-stop', b'$(probe_expand_guard $$(error native-stop))', b'', error=b'native-stop')
    add('eval-explicit-location', b'$(probe_eval_location $$(error native-stop))', b'', error=b'native-stop', stderr=b'module-eval.mk:23: *** native-stop.  Stop.\n')
    add('eval-include-location', b'$(probe_eval_guard include child.mk)', b'', extra={'child.mk': b'$(error native-stop)\n'}, error=b'native-stop', stderr=b'child.mk:1: *** native-stop.  Stop.\n')
    add('setup-once-witness', b'$(probe_where )', b'Makefile:1', loads=b'load ./probe_api.so\nload ./probe_api.so\n', modules=(Module(defines=('PROBE_TRACE_SETUP',)),), seen=b'anchor\nsetup\nbefore\n')
    for optional in (False, True):
        prefix = b'-load ' if optional else b'load '
        add('gpl-marker-' + str(optional).lower(), b'', b'', loads=prefix + b'modules/not_gpl.so\n', modules=(Module('not_gpl.c', 'modules/not_gpl.so'),), error=b'Loaded object modules/not_gpl.so is not declared to be GPL compatible', error_line=1, seen=b'anchor\n')
        symbol = b'Failed to load symbol no_setup_gmk_setup from modules/no_setup.so: modules/no_setup.so: undefined symbol: no_setup_gmk_setup'
        add('setup-symbol-' + str(optional).lower(), b'', b'', loads=prefix + b'modules/no_setup.so\n', modules=(Module('no_setup.c', 'modules/no_setup.so'),), error=symbol, error_line=1, seen=b'anchor\n')
    missing = b'Makefile:1: modules/missing.so: cannot open shared object file: No such file or directory\nMakefile:1: *** modules/missing.so: failed to load.  Stop.\n'
    add('missing-required', b'', b'', loads=b'load modules/missing.so\n', modules=(), error=b'missing', stderr=missing, seen=b'anchor\n')
    add('missing-optional', b'done', b'done', loads=b'-load modules/missing.so\n', modules=(), loaded=b'', exit_code=2, stderr=b"make: *** No rule to make target 'modules/missing.so'.  Stop.\n")
    add('missing-optional-then-required', b'done', b'done', loads=b'-load modules/missing.so\nload modules/missing.so\n', modules=(), loaded=b'', exit_code=2, stderr=b"make: *** No rule to make target 'modules/missing.so'.  Stop.\n")
    for kind, value in (('target', b'VALUE'), ('automatic', b'@')):
        body = b'load probe_api.so\n$(info LOADED=[$(.LOADED)])\nall: VALUE = target\nall:\n\t@$(file >>seen,before)printf "%s\\n" "RESULT=[$(probe_expand $$(' + value + b'))]"\n'
        expected = b'LOADED=[probe_api.so]\nRESULT=[' + (b'target' if kind == 'target' else b'all') + b']\n'
        result.append(Model('native-recipe-' + kind, {'Makefile': body, 'seen': b'anchor\n'}, {'exit_code': 0, 'stdout_hex': expected.hex(), 'stderr_hex': ''}, b'anchor\nbefore\n'))
    result.extend(lifecycle_models())
    result.extend(setup_result_models())
    result.extend(loader_debug_models())
    result.extend(additional_models())
    result.extend(stdio_models())
    result.extend(registration_models())
    result.extend(final_boundary_models())
    result.extend(deep_models())
    result.extend(source_provenance_models())
    result.extend(startup_exit_models())
    result.extend(target_read_models())
    result.extend(secondary_scope_models())
    result.extend(retained_double_branch_models())
    ids = [model.id for model in result]
    if len(ids) != len(set(ids)) or any(not re.fullmatch(r'[a-z0-9][a-z0-9-]*', ident) for ident in ids):
        raise h.HarnessError('duplicate or invalid native model IDs')
    return result



def lifecycle_models() -> list[Model]:
    result = []
    include = (b'.DEFAULT_GOAL := all\nload probe_api.so\ninclude generated.mk\n'
               b'$(file >>seen,read-$(MAKE_RESTARTS))\n'
               b'$(info READ=[$(MAKE_RESTARTS)] LOCATION=[$(probe_where )] LOADED=[$(.LOADED)])\n'
               b'.PHONY: all\nall:;@:\ngenerated.mk:\n\t@printf "VALUE := rebuilt\\n" >$@\n')
    include_out = (b'READ=[] LOCATION=[Makefile:2] LOADED=[probe_api.so]\n'
                   b'READ=[1] LOCATION=[Makefile:2] LOADED=[probe_api.so]\n')
    witness = b'anchor\nsetup\nread-\nsetup\nread-1\n'
    result.append(Model('native-remade-include-reruns-setup', {'Makefile': include, 'seen': b'anchor\n'},
                        {'exit_code': 0, 'stdout_hex': include_out.hex(), 'stderr_hex': ''}, witness,
                        (Module(defines=('PROBE_TRACE_SETUP',)),)))
    rebuilt = (b'.DEFAULT_GOAL := all\nload probe_api.so\n'
               b'$(file >>seen,read-$(MAKE_RESTARTS))\n'
               b'$(info READ=[$(MAKE_RESTARTS)] VERSION=[$(probe_version )] LOADED=[$(.LOADED)])\n'
               b'.PHONY: all\nall:;@:\nprobe_api.so: replacement.so\n'
               b'\t@cp replacement.so new-module.so\n\t@mv new-module.so $@\n')
    rebuilt_out = (b'READ=[] VERSION=[version-1] LOADED=[probe_api.so]\n'
                   b'READ=[1] VERSION=[version-2] LOADED=[probe_api.so]\n')
    result.append(Model('native-remade-module-reloads-callbacks', {'Makefile': rebuilt, 'seen': b'anchor\n'},
                        {'exit_code': 0, 'stdout_hex': rebuilt_out.hex(), 'stderr_hex': ''}, witness,
                        (Module(defines=('PROBE_TRACE_SETUP', 'PROBE_VERSION=1')),
                         Module(filename='replacement.so', defines=('PROBE_TRACE_SETUP', 'PROBE_VERSION=2'))),
                        mtimes_ns={'probe_api.so': 1_000_000_000, 'replacement.so': 2_000_000_000}))
    parent = (b'$(file >>seen,before)\n.PHONY: all child after\nall: child after\nchild:\n'
              b'\t+@$(MAKE) --no-print-directory -f native-child.mk\nafter:\n\t@:\n')
    child = (b'load probe_api.so\n.PHONY: all worker bad\nall: worker bad\nworker:\n'
             b'\t@sh worker.sh\nbad:\n\t@$(probe_wait_fatal )$(file >>seen,forbidden-recipe):\n')
    worker = (b'printf "worker-start\\n" >>seen\nprintf started >child-started\n'
              b'count=0\nwhile [ ! -f callback-entered ]; do\n'
              b'  count=$((count + 1))\n  [ "$count" -lt 200 ] || exit 95\n  sleep 0.01\ndone\n'
              b'printf "worker-done\\n" >>seen\n')
    diagnostic = (b'module-worker.mk:17: *** native-worker-stop.  Stop.\n'
                  b'make[1]: *** Waiting for unfinished jobs....\n'
                  b'make: *** [Makefile:5: child] Error 2\n'
                  b"make: Target 'all' not remade because of errors.\n")
    for transport in ('fifo', 'pipe'):
        for sync in ('none', 'target'):
            result.append(Model('native-fatal-worker-' + transport + '-' + sync,
                                {'Makefile': parent, 'native-child.mk': child, 'worker.sh': worker, 'seen': b'anchor\n'},
                                {'exit_code': 2, 'stdout_hex': '', 'stderr_hex': diagnostic.hex()},
                                b'anchor\nbefore\nworker-start\ncallback-entered\nworker-done\n',
                                args=('-rR', '--no-print-directory', '-k', '-j2', '--jobserver-style=' + transport, '-O' + sync)))
    return result



def setup_result_models() -> list[Model]:
    header = b'.DEFAULT_GOAL := all\n.LOADED := seeded\n'
    body = (b'$(file >>seen,read-$(MAKE_RESTARTS))\n'
            b'$(info read=[$(MAKE_RESTARTS)] loaded=[$(.LOADED)] image=[$(probe_image )])\n'
            b'.PHONY: all\nall:\n\t@$(file >>seen,goal-$(MAKE_RESTARTS)):\n'
            b'probe.so: replacement.so\n'
            b'\t@$(file >>seen,rebuild)cp replacement.so stage.so\n'
            b'\t@mv stage.so probe.so\n')
    read_old = b'read=[] loaded=[seeded probe.so] image=[old]\n'
    read_new = b'read=[1] loaded=[seeded probe.so] image=[new]\n'
    old_read = b'anchor\nctor-old\nsetup-old\nread-\n'
    no_remake = old_read + b'goal-\ndtor-old\n'
    remade = old_read + b'dtor-old\nrebuild\nctor-new\nsetup-new\nread-1\ngoal-1\ndtor-new\n'
    cases = (
        ('negative-one', -1, b'load ./probe.so\n', (), read_old, no_remake),
        ('negative-one-optional-explicit-and-repeat', -1,
         b'-load probe.so(alternate_setup)\nload ./probe.so(unused_setup)\n', (), read_old, no_remake),
        ('negative-one-explicit-goal', -1, b'load probe.so\n', ('probe.so',),
         read_old, old_read + b'dtor-old\nrebuild\n'),
        ('negative-two-remakes', -2, b'load probe.so\n', (), read_old + read_new, remade),
        ('positive-one-remakes', 1, b'load probe.so\n', (), read_old + read_new, remade),
    )
    older = 1_600_000_000_000_000_000
    result = []
    for ident, status, loads, args, stdout, seen in cases:
        modules = (Module('setup_result.c', 'probe.so', ('IMAGE="old"', f'SETUP_RESULT={status}')),
                   Module('setup_result.c', 'replacement.so', ('IMAGE="new"', 'SETUP_RESULT=1')))
        result.append(Model('native-setup-result-' + ident,
                            {'Makefile': header + loads + body, 'seen': b'anchor\n'},
                            {'exit_code': 0, 'stdout_hex': stdout.hex(), 'stderr_hex': ''}, seen, modules,
                            args=('-rR', '--no-print-directory', *args),
                            mtimes_ns={'probe.so': older, 'replacement.so': older + 2_000_000_000}))
    return result


def loader_debug_models() -> list[Model]:
    banner = (b'GNU Make 4.4.1\n'
              b'Built for x86_64-pc-linux-gnu\n'
              b'Copyright (C) 1988-2023 Free Software Foundation, Inc.\n'
              b'License GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>\n'
              b'This is free software: you are free to change and redistribute it.\n'
              b'There is NO WARRANTY, to the extent permitted by law.\n')
    basic = banner + b'Reading makefiles...\n'
    verbose = basic + b"Reading makefile 'Makefile'...\n"
    update = b'Updating makefiles....\n'
    goals = b'Updating goal targets....\n'
    all = (update + goals + b"Considering target file 'all'.\n File 'all' does not exist.\n"
           b"Finished prerequisites of target file 'all'.\nMust remake target 'all'.\n"
           b"Successfully remade target file 'all'.\n")
    prologue = b'.DEFAULT_GOAL := all\n'
    run_all = b'.PHONY: all\nall:;@$(file >>seen,goal):\n'
    read = b'$(info read loaded=[$(.LOADED)])\n'
    lookup = b'Loading symbol probe_gmk_setup from probe.so\n'
    opened = b'constructor stdout\nLoaded shared object probe.so\n'
    setup = b'setup stdout\n'
    ready = b'read loaded=[probe.so]\n'

    cases = (
        ('verbose-success', ('--debug=v',), prologue + b'load ./probe.so\n' + read + run_all,
         True, False, 0, verbose + lookup + opened + setup + ready + all, b'',
         b'anchor\nctor\nsetup\ngoal\ndtor\n'),
        ('verbose-global-symbol-and-repeat', ('--debug=v',),
         prologue + b'load probe.so\nload ./probe.so(unused_setup)\nload alias.so(probe_gmk_setup)\n' + read + run_all,
         True, True, 0, verbose + lookup + opened + setup +
         b'Loading symbol probe_gmk_setup from alias.so\nsetup stdout\nread loaded=[probe.so alias.so]\n' + all,
         b'', b'anchor\nctor\nsetup\nsetup\ngoal\ndtor\n'),
        ('verbose-bad-symbol', ('--debug=v',), b'load probe.so(absent_setup)\n' + read + run_all,
         True, False, 2, verbose + b'Loading symbol absent_setup from probe.so\n' + opened,
         b'Makefile:1: *** Failed to load symbol absent_setup from probe.so: ./probe.so: undefined symbol: absent_setup.  Stop.\n',
         b'anchor\nctor\ndtor\n'),
        ('basic-optional-missing', ('--debug=b',), prologue + b'-load absent.so\n' + read + run_all,
         False, False, 2, basic + b'absent.so: cannot open shared object file: No such file or directory\n' +
         b'read loaded=[]\n' + update,
         b"make: *** No rule to make target 'absent.so'.  Stop.\n", b'anchor\n'),
        ('verbose-required-missing', ('--debug=v',), b'load absent.so\n' + read + run_all,
         False, False, 2, verbose + b'Loading symbol absent_gmk_setup from absent.so\n',
         b'Makefile:1: absent.so: cannot open shared object file: No such file or directory\n'
         b'Makefile:1: *** absent.so: failed to load.  Stop.\n', b'anchor\n'),
        ('verbose-explicit-unload', ('--debug=v', '-B', 'probe.so'), prologue + b'load probe.so\n' + read +
         run_all + b'probe.so:;@$(file >>seen,rebuild):\n',
         True, False, 0, verbose + lookup + opened + setup + ready + update + goals +
         b"Considering target file 'probe.so'.\nFinished prerequisites of target file 'probe.so'.\n"
         b"Making 'probe.so' due to always-make flag.\nMust remake target 'probe.so'.\n"
         b"Unloading shared object probe.so\nSuccessfully remade target file 'probe.so'.\n", b'',
         b'anchor\nctor\nsetup\ndtor\nrebuild\n'),
    )
    result = [
        Model('native-loader-debug-' + ident, {'Makefile': makefile, 'seen': b'anchor\n'},
              {'exit_code': status, 'stdout_hex': stdout.hex(), 'stderr_hex': stderr.hex()}, seen,
              (Module('loader_debug.c', 'probe.so'),) if dso else (),
              args=('-rR', '--no-print-directory', *args),
              symlinks={'alias.so': 'probe.so'} if alias else {})
        for ident, args, makefile, dso, alias, status, stdout, stderr, seen in cases
    ]
    cache_load = (b'before := $(wildcard *.input missing/*.input)\n'
                  b'load ./probe_api.so\n')
    cache_tail = b'after := $(wildcard *.input missing/*.input)\nall: ; @:\n'
    cache_read = (verbose + b'Loading symbol probe_api_gmk_setup from probe_api.so\n'
                  b'Loaded shared object probe_api.so\n')

    def stale(command: int) -> bytes:
        return b''.join(
            b'Directory %s cache invalidated (count 1 != command %d)\n' % (path, command)
            for path in (b'.', b'missing')
        )

    for ident, body, output in (
        ('registration', b'', b''),
        ('pure-and-null',
         b'$(info a=$(probe_args one))\n$(info b=$(probe_null ))\n',
         b'a=argc=1[one]\nb=\n' + stale(3)),
        ('eager-write', b'$(info a=$(probe_args $(file >>seen)))\n',
         b'a=argc=1[]\n' + stale(3)),
        ('nested-expansion',
         b'$(info a=$(probe_expand $$(wildcard *.input missing/*.input)))\n',
         stale(2) + b'a=file.input\n'),
        ('call-dispatch', b'$(info a=$(call probe_args,one,two))\n',
         b'a=argc=2[one][two]\n' + stale(2)),
    ):
        result.append(Model(
            'native-directory-epoch-' + ident,
            {'Makefile': cache_load + body + cache_tail, 'seen': b'anchor\n',
             'file.input': b'file'},
            {'exit_code': 0, 'stdout_hex': (cache_read + output + all).hex(),
             'stderr_hex': ''},
            b'anchor\n', args=('-rR', '--no-print-directory', '--debug=v', 'all'),
        ))
    return result


def additional_models() -> list[Model]:
    result = []
    add = lambda *args, **kwargs: result.append(expression_model(*args, **kwargs))
    add('call-maximum-pair', b'$(call probe_pair,one,two,three)', b'argc=3[one][two][three]')
    add('call-maximum-raw', b'$(call probe_raw,one,two,three)', b'argc=3[one][two][three]')
    add('eval-null-warning-read', b'$(probe_eval_guard $$(warning native-warning))', b'original',
        seen=b'anchor\nbefore\neval-returned\n', stderr=b'Makefile:4: native-warning\n')
    body = b'load probe_api.so\nVALUE = original\nall:\n\t@$(file >>seen,before)printf "%s\\n" "RESULT=[$(probe_eval_guard $$(warning native-warning))]"\n'
    result.append(Model('native-eval-null-warning-recipe', {'Makefile': body, 'seen': b'anchor\n'},
                        {'exit_code': 0, 'stdout_hex': b'RESULT=[original]\n'.hex(),
                         'stderr_hex': b'Makefile:4: native-warning\n'.hex()}, b'anchor\nbefore\neval-returned\n'))
    add('constructor-eval', b'$(CONSTRUCTOR_SEEN)', b'ready', modules=(Module(defines=('PROBE_CONSTRUCTOR',)),),
        seen=b'anchor\nconstructor-enter\nconstructor-return\nbefore\n')
    add('constructor-fatal-stop', b'', b'', modules=(Module(defines=('PROBE_CONSTRUCTOR_FATAL',)),),
        error=b'constructor-stop', error_line=1, seen=b'anchor\nconstructor-enter\n')
    add('constructor-nested-load', b'$(probe_args nested)', b'argc=1[nested]',
        modules=(Module(defines=('PROBE_CONSTRUCTOR_NESTED',)), Module(filename='inner.so', defines=('PROBE_INNER',))),
        loaded=b'inner.so probe_api.so', seen=b'anchor\nconstructor-enter\nnested-eval\nconstructor-return\nbefore\n')
    add('lifecycle-normal-exit', b'$(probe_args x)', b'argc=1[x]',
        modules=(Module(defines=('PROBE_TRACE_SETUP', 'PROBE_LIFECYCLE')),),
        seen=b'anchor\nconstructor\nsetup\nbefore\ndestructor\n')
    # GNU exec replaces an unchanged image without calling its destructor. A
    # changed loaded module is explicitly closed before its rebuild recipe.
    include, rebuilt = lifecycle_models()[:2]
    result.append(replace(include, id='native-lifecycle-include-restart',
                          modules=(Module(defines=('PROBE_TRACE_SETUP', 'PROBE_LIFECYCLE')),),
                          seen=b'anchor\nconstructor\nsetup\nread-\nconstructor\nsetup\nread-1\ndestructor\n'))
    versioned = (Module(defines=('PROBE_TRACE_SETUP', 'PROBE_LIFECYCLE', 'PROBE_VERSION=1')),
                 Module(filename='replacement.so', defines=('PROBE_TRACE_SETUP', 'PROBE_LIFECYCLE', 'PROBE_VERSION=2')))
    result.append(replace(rebuilt, id='native-lifecycle-module-rebuild', modules=versioned,
                          seen=b'anchor\nconstructor\nsetup\nread-\ndestructor\nconstructor\nsetup\nread-1\ndestructor\n'))
    alias = expression_model('alias-setup-lifetime', b'$(probe_where )', b'Makefile:2',
                             loads=b'load probe_api.so\nload aliases/probe_api.so\n',
                             loaded=b'probe_api.so aliases/probe_api.so',
                             modules=(Module(defines=('PROBE_TRACE_SETUP', 'PROBE_LIFECYCLE')),),
                             seen=b'anchor\nconstructor\nsetup\nsetup\nbefore\ndestructor\n')
    result.append(replace(alias, symlinks={'aliases/probe_api.so': '../probe_api.so'}))
    body = rebuilt.files['Makefile'].replace(b'load probe_api.so\n', b'load probe_api.so\nload aliases/probe_api.so\n')
    expected = dict(rebuilt.expected)
    expected['stdout_hex'] = bytes.fromhex(expected['stdout_hex']).replace(b'LOADED=[probe_api.so]', b'LOADED=[probe_api.so aliases/probe_api.so]').hex()
    result.append(replace(rebuilt, id='native-alias-rebuild-lifetime', files={'Makefile': body, 'seen': b'anchor\n'},
                          expected=expected, modules=versioned, symlinks={'aliases/probe_api.so': '../probe_api.so'},
                          seen=b'anchor\nconstructor\nsetup\nsetup\nread-\ndestructor\nconstructor\nsetup\nsetup\nread-1\ndestructor\n'))
    for kind, expression, error in (
        ('success', b'$(probe_args x)', None),
        ('core-fatal', b'$(error main-stop)', b'main-stop'),
        ('native-fatal', b'$(probe_eval_guard $$(error native-stop))', b'native-stop'),
    ):
        model = expression_model('destructor-api-' + kind, expression, b'argc=1[x]',
                                 modules=(Module(defines=('PROBE_EXIT_API',)),), error=error,
                                 seen=b'anchor\nbefore\ndestructor-value=original\ndestructor-eval=updated\n')
        expected = dict(model.expected)
        # GNU close_stdout runs before library finalizers; their stdout is closed.
        expected['stderr_hex'] = (bytes.fromhex(expected['stderr_hex']) + b'destructor-stderr\n').hex()
        result.append(replace(model, expected=expected))
    return result


def stdio_models() -> list[Model]:
    result = []
    for phase in ('read', 'recipe'):
        for mode in ('none', 'target', 'line', 'recurse'):
            for output in ('separate', 'merged'):
                ident = 'native-stdio-' + phase + '-' + mode + '-' + output
                body = (b'load probe_stdio.so\nVALUE = original\n$(file >>seen,before)\n$(info RESULT=[$(probe_stdio )])\n$(info LOADED=[$(.LOADED)])\nall:;@:\n' if phase == 'read' else
                        b'load probe_stdio.so\n$(info LOADED=[$(.LOADED)])\nall:\n\t@$(file >>seen,before)printf "%s\\n" "RESULT=[$(probe_stdio )]"\n')
                setup_raw, setup_info = b'setup-partial-tail\n', b'setup-info\n'
                callback_raw, callback_info = b'callback-partial-tail\n', b'callback-info\n'
                warning_setup, warning_callback = b'Makefile:1: setup-warning\n', b'Makefile:4: callback-warning\n'
                loaded, value = b'LOADED=[probe_stdio.so]\n', b'RESULT=[stdio]\n'
                reading_buffered = mode in ('target', 'line')
                if output == 'separate':
                    stdout = ((setup_raw + callback_raw + setup_info + callback_info + value + loaded)
                              if phase == 'read' and reading_buffered else
                              (setup_raw + setup_info + callback_raw + callback_info + value + loaded)
                              if phase == 'read' else
                              (setup_raw + setup_info + loaded + callback_raw + callback_info + value))
                    stderr = warning_setup + warning_callback
                else:
                    if phase == 'read':
                        stdout = ((setup_raw + callback_raw + warning_setup + setup_info + warning_callback + callback_info + value + loaded)
                                  if reading_buffered else
                                  (warning_setup + setup_raw + setup_info + warning_callback + callback_raw + callback_info + value + loaded))
                    else:
                        setup = setup_raw + warning_setup + setup_info if reading_buffered else warning_setup + setup_raw + setup_info
                        callback = callback_raw + warning_callback + callback_info if mode != 'none' else warning_callback + callback_raw + callback_info
                        stdout = setup + loaded + callback + value
                    stderr = b''
                result.append(Model(ident, {'Makefile': body, 'seen': b'anchor\n'},
                                    {'exit_code': 0, 'stdout_hex': stdout.hex(), 'stderr_hex': stderr.hex()},
                                    b'anchor\nbefore\n', (Module('stdio.c', 'probe_stdio.so'),),
                                    args=('-rR', '--no-print-directory', '-j2', '-O' + mode), output=output))
    return result


def registration_models() -> list[Model]:
    models=[]
    def add(ident,name,*,message=None,minimum=0,maximum=1,flags=0,invoke=True,phase='read'):
     expression=b'$('+name+b' $(file >>seen,argument)x)'if invoke else b'registered'
     value=b'accepted'if invoke else b'registered'
     environment={'GNUMAKEFLAGS':'','PROBE_REG_NAME':os.fsdecode(name),'PROBE_REG_MIN':str(minimum),'PROBE_REG_MAX':str(maximum),'PROBE_REG_FLAGS':str(flags)}
     files={'seen':b'anchor\n'};where=b'Makefile:1';modules=(Module('registration.c','registration.so'),)
     if phase=='read':
      files['Makefile']=b'load registration.so\n$(file >>seen,body)\n$(info RESULT=['+expression+b'])\nall:;@:\n';before=b'anchor\n';after=b'body\n'
     elif phase=='include':
      files['Makefile']=b'$(file >>seen,outer)\ninclude child.mk\nall:;@:\n';files['child.mk']=b'# retained location\nload registration.so\n$(file >>seen,body)\n$(info RESULT=['+expression+b'])\n';where=b'child.mk:2';before=b'anchor\nouter\n';after=b'body\n'
     elif phase=='recipe':
      files['Makefile']=b'load probe_api.so\n.PHONY: all\nall:\n\t@$(file >>seen,body)$(eval load registration.so)printf "%s\\n" "RESULT=['+expression+b']"\n';where=b'Makefile:4';before=b'anchor\nbody\n';after=b'';modules=(Module(),*modules)
     else:raise AssertionError(phase)
     seen=before+b'register-before\n'
     if message is None:seen+=b'register-after\n'+after+(b'argument\n'if invoke and not(flags&1)else b'')+(b'called\n'if invoke else b'')
     expected={'exit_code':2 if message is not None else 0,'stdout_hex':(b''if message is not None else b'RESULT=['+value+b']\n').hex(),'stderr_hex':(where+b': *** '+message+b'.  Stop.\n'if message is not None else b'').hex()}
     models.append(Model('native-registration-'+ident+'-'+phase,files,expected,seen,modules,environment=environment))
    for ident,name in [('alpha',b'native_registered'),('digit',b'9native'),('hyphen',b'-native'),('underscore',b'_native'),('internal-dot',b'native.dot'),('one',b'n'),('length255',b'n'*255)]:add(ident,name)
    for ident,name,kind in [('empty',b'','empty'),('first-space',b' native','empty'),('first-punctuation',b'!native','empty'),('first-colon',b':native','empty'),('first-highbyte',b'\xffnative','empty'),('dot',b'.','invalid'),('leading-dot',b'.native','invalid'),('space',b'native name','invalid'),('slash',b'native/name','invalid'),('colon',b'native:colon','invalid'),('dollar',b'native$tail','invalid'),('body-highbyte',b'native\xff','invalid'),('length256',b'n'*256,'long'),('length256-invalid',b'n'*256+b'!','invalid')]:
     message=b'Empty function name'if kind=='empty'else(b'Invalid function name: 'if kind=='invalid'else b'Function name too long: ')+name
     add(ident,name,message=message)
    for ident,minimum,maximum,message,invoke in [('min256',256,0,b'Invalid minimum argument count (256) for function native_registered',False),('max256',0,256,b'Invalid maximum argument count (256) for function native_registered',False),('max-below-min',2,1,b'Invalid maximum argument count (1) for function native_registered',False),('min255-unlimited',255,0,None,False),('min255-max255',255,255,None,False),('max-unlimited',0,0,None,True),('max255',0,255,None,True)]:add(ident,b'native_registered',minimum=minimum,maximum=maximum,message=message,invoke=invoke)
    add('first-invalid-priority',b'!native',minimum=256,maximum=256,message=b'Empty function name')
    add('name-length-priority',b'n'*256,minimum=256,maximum=256,message=b'Function name too long: '+b'n'*256)
    for flags in (1,2,0xffffffff):add('flags-'+str(flags),b'native_registered',flags=flags)
    for phase in('include','recipe'):
     add('first-invalid',b'!native',message=b'Empty function name',phase=phase)
     add('body-invalid',b'native!',message=b'Invalid function name: native!',phase=phase)
     add('valid',b'native_registered',phase=phase)
     add('min-invalid',b'native_registered',minimum=256,maximum=0,message=b'Invalid minimum argument count (256) for function native_registered',phase=phase)
    return models



def final_boundary_models() -> list[Model]:
    models = []
    for optional in (False, True):
        models.append(expression_model('global-symbol-missing-' + str(optional).lower(), b'$(probe_where )', b'Makefile:2',
                      loads=b'load probe_api.so\n' + (b'-load ' if optional else b'load ') + b'missing-alias.so(probe_api_gmk_setup)\n',
                      loaded=b'probe_api.so missing-alias.so', modules=(Module(defines=('PROBE_TRACE_SETUP',)),),
                      seen=b'anchor\nsetup\nsetup\nbefore\n', exit_code=2,
                      stderr=b"make: *** No rule to make target 'missing-alias.so'.  Stop.\n"))
    for line in (0, 1, 23, 42, 2**32 - 1, 2**64 - 1):
        models.append(Model('native-explicit-line-' + str(line),
                            {'Makefile': b'load ./probe_line.so\n.PHONY: all\nall:;@:\n', 'seen': b'anchor\n'},
                            {'exit_code': 0, 'stdout_hex': '',
                             'stderr_hex': f'native-lines.mk:{line}: first\nnative-lines.mk:{line}: second\n'.encode().hex()},
                            b'anchor\n', (Module('probe_line.c', 'probe_line.so'),),
                            environment={'GNUMAKEFLAGS': '', 'PROBE_LINE': str(line)}))
    for alternate in (False, True):
        alias = b'aliases/alias.so' if alternate else b'aliases/probe_api.so'
        body = (b'.DEFAULT_GOAL := all\nload probe_api.so\nload ' + alias + b'(probe_api_gmk_setup)\n'
                b'$(file >>seen,read)\n.PHONY: all FORCE\nall:\n'
                b'\t@$(file >>seen,recipe)printf "%s\\n" "RESULT=[$(probe_where )] LOADED=[$(.LOADED)] RESTART=[$(MAKE_RESTARTS)]"\n'
                b'ifeq ($(MAKE_RESTARTS),)\n' + alias + b': FORCE\n\t@:\nFORCE:\nendif\n')
        if alternate:
            expected = {'exit_code': 2, 'stdout_hex': '', 'stderr_hex': b'Makefile:3: *** Failed to load symbol alias_gmk_setup from aliases/alias.so: ./probe_api.so: undefined symbol: alias_gmk_setup.  Stop.\n'.hex()}
            seen = b'anchor\nconstructor\nsetup\nsetup\nread\ndestructor\n'
        else:
            expected = {'exit_code': 0, 'stdout_hex': b'RESULT=[Makefile:3] LOADED=[probe_api.so aliases/probe_api.so aliases/probe_api.so] RESTART=[]\n'.hex(), 'stderr_hex': ''}
            seen = b'anchor\nconstructor\nsetup\nsetup\nread\nsetup\nrecipe\ndestructor\n'
        models.append(Model('native-alias-noop-' + ('default-symbol-error' if alternate else 'reload'),
                            {'Makefile': body, 'seen': b'anchor\n'}, expected, seen,
                            (Module(defines=('PROBE_TRACE_SETUP', 'PROBE_LIFECYCLE')),),
                            symlinks={alias.decode(): '../probe_api.so'}))
    for mode in ('none', 'target'):
        model = expression_model('destructor-fd-reuse-' + mode, b'$(probe_args x)', b'argc=1[x]',
                                 modules=(Module(defines=('PROBE_EXIT_FD_REUSE',)),),
                                 args=('-j2', '-O' + mode), seen=b'anchor\nbefore\ndestructor-fd-mark\n')
        models.append(model)
    models.append(Model('native-primary-failure-finalizer-retry',
                        {'Makefile': b'load ./retry_module.so\n$(file >>seen,forbidden-read)\nall:;@:\n', 'seen': b'anchor\n'},
                        {'exit_code': 2, 'stdout_hex': '',
                         'stderr_hex': b'Makefile:1: *** retry_module.so: failed to load.  Stop.\n'.hex()},
                        b'anchor\nsetup-1\nfinal-enter\nsetup-2\nfinal-after\n',
                        (Module('retry_module.c', 'retry_module.so'),)))
    return models



def deep_models() -> list[Model]:
    rows = [(f'finite-{depth}', b'VALUE = global\n',
             b'$(foreach VALUE,local,$(deep ' + str(depth).encode() + b'))',
             b'local', 0, b'') for depth in (32, 96, 128, 256, 1024)]
    rows.extend([
        ('fatal-leaf', b'VALUE = $(error deep-stop)$(file >>seen,forbidden)\n',
         b'$(deep 256)$(file >>seen,after)', b'', 2, b'Makefile:3: *** deep-stop.  Stop.\n'),
        ('active-cycle', b'VALUE = $(deep 1)\n', b'$(VALUE)', b'', 2,
         b"Makefile:1: *** Recursive variable 'VALUE' references itself (eventually).  Stop.\n"),
    ])
    result = []
    for ident, preamble, expression, value, status, stderr in rows:
        body = preamble + b'load ./probe_deep.so\n$(info RESULT=[' + expression + b'])\n.PHONY: all\nall:;@:\n'
        stdout = b'RESULT=[' + value + b']\n' if status == 0 else b''
        result.append(Model('native-deep-' + ident, {'Makefile': body, 'seen': b'anchor\n'},
                            {'exit_code': status, 'stdout_hex': stdout.hex(), 'stderr_hex': stderr.hex()},
                            b'anchor\nleaf\n', (Module('probe_deep.c', 'probe_deep.so'),)))
    return result



def source_provenance_models() -> list[Model]:
    data = tomllib.loads((SOURCES / 'source-provenance.toml').read_text())
    if data.get('schema_version') != 1:
        raise h.HarnessError('unsupported source provenance model schema')
    modules = {
        'provenance_driver.so': Module('provenance_driver.c', 'provenance_driver.so'),
        'prov_probe.so': Module('provenance_probe.c', 'prov_probe.so'),
        'prov_reload.so': Module('provenance_probe.c', 'prov_reload.so', ('PROV_RELOAD',)),
    }
    return [Model(row['id'], {name: bytes.fromhex(value) for name, value in row['files'].items()},
                  row['expected'], bytes.fromhex(row['expected_seen']),
                  tuple(modules[name] for name in row['modules']), args=tuple(row['args']),
                  environment=row['environment']) for row in data['models']]



def startup_exit_models() -> list[Model]:
    data = tomllib.loads((SOURCES / 'startup-exits.toml').read_text())
    if data.get('schema_version') != 1:
        raise h.HarnessError('unsupported startup exit model schema')
    return [Model(row['id'], {name: bytes.fromhex(value) for name, value in row['files'].items()},
                  row['expected'], bytes.fromhex(row['expected_seen']),
                  tuple(Module('startup_probe.c', name) for name in row['modules']),
                  args=tuple(row['args']), environment=row['environment'])
            for row in data['models']]


def target_read_models() -> list[Model]:
    prefix = b'''load probe_api.so
V := global
all: left right caller
left: V := first
right: V := second
left right: dest
dest:;@$(info selected=[$(V)|$@]):
'''
    retained_call = b'$(probe_eval_guard dest: RESULT := $$(info retained=[$$(V)|$$@])$$(file >>seen,retained))'
    active_prefix = b'''load probe_api.so
V := global
all: V := parent
all: caller
'''
    active_call = b'$(probe_eval_guard caller: RESULT := $$(info active=[$$(V)|$$@])$$(file >>seen,active))'
    child = b'''caller:;@printf 'child=[%s]\\n' "$$TRIGGER"
'''
    def model(name, source, stdout, marks, *, args=()):
        return Model('native-target-read-' + name,
                       {'Makefile': source, 'seen': b'anchor\n'},
                       {'exit_code': 0, 'stdout_hex': stdout.hex(), 'stderr_hex': ''},
                       b'anchor\n' + marks,
                       args=('-rR', '--no-print-directory', *args))
    return [
        model('direct-active', active_prefix + b'caller:;@' + active_call + b':\n',
              b'active=[parent|caller]\n', b'active\neval-returned\n'),
        model('direct-retained', prefix + b'caller:;@' + retained_call + b':\n',
              b'selected=[first|dest]\nretained=[second|dest]\n', b'retained\neval-returned\n'),
        model('direct-nested-shell-locals', b'''load probe_api.so
all: dest caller
dest:;@$(shell /bin/sh -c 'exit 7'):
caller:;@$(shell /bin/sh -c 'exit 11')$(probe_eval_guard dest: OUTER := $$(eval caller: INNER := $$$$(info inside=[$$$$(.SHELLSTATUS)|$$$$@]))$$(info retained=[$$(.SHELLSTATUS)|$$@]))$(info caller=[$(.SHELLSTATUS)|$@]):
''', b'inside=[11|caller]\nretained=[7|dest]\ncaller=[11|caller]\n', b'eval-returned\n'),
        model('environment-retained', prefix + b'caller: export TRIGGER = ' + retained_call + b'ready\n' + child,
              b'selected=[first|dest]\nretained=[second|dest]\nchild=[ready]\n', b'retained\neval-returned\n'),
        model('environment-builtin-control', prefix + b'caller: export TRIGGER = ' +
              retained_call.replace(b'probe_eval_guard', b'eval') + b'ready\n' + child,
              b'selected=[first|dest]\nretained=[second|dest]\nchild=[ready]\n', b'retained\n'),
        model('environment-active', active_prefix + b'caller: export TRIGGER = ' + active_call + b'ready\n' + child,
              b'active=[parent|caller]\nchild=[ready]\n', b'active\neval-returned\n'),
        model('environment-shell-locals', b'''load probe_api.so
all: dest caller
dest:;@$(shell /bin/sh -c 'exit 7'):
caller: export TRIGGER = $(probe_eval_guard dest: FIRST := $$(info before=[$$(.SHELLSTATUS)|$$@])$$(file >>seen,shell)$$(shell /bin/sh -c 'exit 13'))$(probe_eval_guard dest: SECOND := $$(info after=[$$(.SHELLSTATUS)|$$@]))ready
''' + child, b'before=[7|dest]\nafter=[13|dest]\nchild=[ready]\n', b'shell\neval-returned\neval-returned\n'),
        model('shell-capture-environment', prefix + b'caller: export TRIGGER = ' + retained_call +
              b'ready\ncaller:;@$(info shell=[$(shell printf \'%s\' "$$TRIGGER")]):\n',
              b'selected=[first|dest]\n:\nretained=[second|dest]\nshell=[ready]\n:\n',
              b'retained\neval-returned\n', args=('-n',)),
    ]


def secondary_scope_models() -> list[Model]:
    header = b'load probe_api.so\n.SECONDEXPANSION:\n'

    def model(name, source, stdout, marks, **files):
        return Model('native-secondary-scope-' + name,
                       {'Makefile': header + source, 'seen': b'anchor\n', **files},
                       {'exit_code': 0, 'stdout_hex': stdout.hex(), 'stderr_hex': ''},
                       b'anchor\n' + marks, args=('-rR', '--no-print-directory'))

    return [
        model('current-explicit', b'''CHECK = $(probe_eval_guard all: RESULT := $$(info active=[$$@|$$<|$$(.SHELLSTATUS)])$$(file >>seen,active))
all: stamp
all: $$(shell /bin/sh -c 'exit 7') $$(CHECK)
all:;@:
stamp:;@:
''', b'active=[all|stamp|7]\n', b'active\neval-returned\n'),
        model('previous-explicit', b'''CHECK = $(probe_eval_guard first: RESULT := $$(info retained=[$$@|$$<|$$(.SHELLSTATUS)])$$(file >>seen,retained))
all: first after
.PHONY: first
first: stamp
first: $$(shell /bin/sh -c 'exit 7')
after: $$(CHECK)
after:;@:
stamp:;@:
''', b'retained=[first|stamp|7]\n', b'retained\neval-returned\n'),
        model('suspended-explicit', b'''CHECK = $(probe_eval_guard first: RESULT := $$(eval after: BACK := $$$$(info caller=[$$$$@|$$$$(.SHELLSTATUS)])$$$$(file >>seen,caller))$$(info retained=[$$@|$$<|$$(.SHELLSTATUS)])$$(file >>seen,retained))
all: first after
.PHONY: first
first: stamp
first: $$(shell /bin/sh -c 'exit 7')
after: $$(shell /bin/sh -c 'exit 11') $$(CHECK)
after:;@$(info recipe=[$@|$(.SHELLSTATUS)]):
stamp:;@:
''', b'caller=[after|11]\nretained=[first|stamp|7]\nrecipe=[after|11]\n',
              b'caller\nretained\neval-returned\n'),
        model('unentered-implicit-intermediate', b'''CHECK = $(probe_eval_guard item.mid: RESULT := $$(info active=[$$@|$$*])$$(file >>seen,active))
all: item.out
all:;@:
%.out: %.mid
	@:
%.mid: %.in $$(CHECK)
	@:
''', b'active=[|]\n', b'active\neval-returned\n', **{'item.in': b''}),
    ]


def retained_double_branch_models() -> list[Model]:
    header = b'''load probe_api.so
.SECONDEXPANSION:
HEAD = $(shell /bin/sh -c 'exit 7')$(file >>seen,head)
NEXT = $(shell /bin/sh -c 'exit 11')$(file >>seen,next)
READ = $(probe_eval_guard shared: RESULT := $$(info retained=[$$@|$$(.SHELLSTATUS)])$$(file >>seen,retained))
all: split
all:;@:
shared:;@$(info first=[$@|$(.SHELLSTATUS)])$(file >>seen,shared):
'''

    def model(name, body, stdout):
        return Model('native-retained-double-branch-' + name,
                            {'Makefile': header + body, 'seen': b'anchor\n'},
                            {'exit_code': 0, 'stdout_hex': stdout.hex(), 'stderr_hex': ''},
                            b'anchor\nhead\nshared\nnext\nretained\neval-returned\n',
                            args=('-rR', '--no-print-directory'))

    return [
        model('live', b'''split:: shared $$(HEAD)
	@:
split:: shared $$(NEXT)
	@$(shell /bin/sh -c 'exit 13')$(READ)$(info outer=[$@|$(.SHELLSTATUS)]):
''', b'first=[shared|7]\nretained=[shared|13]\nouter=[split|13]\n'),
        model('child-export', b'''split:: shared $$(HEAD)
split:: shared $$(NEXT)
	@$(eval export TRIGGER = $$(READ)ready)printf 'child=[%s]\\n' "$$TRIGGER"
''', b'first=[shared|7]\nretained=[shared|11]\nchild=[ready]\n'),
    ]


def seen_matches(model: Model, observation: h.Observation) -> bool:
    entry = observation.filesystem.get('work/seen', {})
    if entry.get('type') != 'file':
        return False
    try:
        return base64.b64decode(entry['content'], validate=True) == model.seen
    except (KeyError, ValueError):
        return False


def assess(model: Model, case: h.Case, observations: list[h.Observation]) -> dict[str, Any]:
    if len(observations) not in (2, 3):
        raise h.HarnessError('native assessment requires two oracle observations and optional candidate')
    expected_errors = [h.expectation_errors(case, observation) for observation in observations]
    witnesses = [seen_matches(model, observation) for observation in observations]
    repeat = h.differences(observations[0], observations[1])
    candidate = h.differences(observations[0], observations[2]) if len(observations) == 3 else None
    bounded = [not observation.limit and not observation.leftover_processes for observation in observations]
    passed = (not any(expected_errors) and all(witnesses) and not repeat and not candidate and all(bounded))
    return {'passed': passed, 'expected_errors': expected_errors, 'seen_matches': witnesses,
            'oracle_repeat_differences': repeat, 'candidate_differences': candidate,
            'bounded': bounded,
            'observations': [dict(h.observation_json(o), stdout_hex=o.stdout.hex(), stderr_hex=o.stderr.hex()) for o in observations]}


def safe_relative(name: str) -> Path:
    path = Path(name)
    if path.is_absolute() or '..' in path.parts or '\0' in name:
        raise h.HarnessError('native fixture path must stay in its disposable input directory')
    return path


def prepare(model: Model, directory: Path, compiler: str) -> tuple[h.Case, list[dict[str, Any]]]:
    directory.mkdir()
    for name, data in model.files.items():
        destination = directory / safe_relative(name)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        os.utime(destination, ns=(INPUT_TIME_NS, INPUT_TIME_NS))
    records = []
    for index, module in enumerate(model.modules):
        source = SOURCES / safe_relative(module.source)
        copied = directory / ('module-' + str(index) + '.c')
        copied.write_bytes(source.read_bytes())
        output = directory / safe_relative(module.filename)
        output.parent.mkdir(parents=True, exist_ok=True)
        command = [compiler, '-std=c11', '-Wall', '-Wextra', '-Werror', '-fPIC', '-shared',
                   *('-D' + value for value in module.defines), str(copied), '-o', str(output)]
        out, err, status, limit, leftovers = h.capture(command, directory,
                                                     {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'LANG': 'C', 'TZ': 'UTC'}, 15)
        record = {'command': command, 'source': module.source,
                  'source_sha256': hashlib.sha256(copied.read_bytes()).hexdigest(),
                  'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'exit_code': status,
                  'limit': limit, 'leftover_processes': leftovers}
        records.append(record)
        if status or limit or leftovers:
            raise h.HarnessError('native module compilation failed: ' + json.dumps(record))
        record['module_sha256'] = hashlib.sha256(output.read_bytes()).hexdigest()
        module_time = INPUT_TIME_NS + 1_000_000_000
        os.utime(output, ns=(module_time, module_time))
    for name, target in model.symlinks.items():
        link = directory / safe_relative(name)
        link.parent.mkdir(parents=True, exist_ok=True)
        link.symlink_to(target)
    h.validate_fixture(directory)
    # Models with a deliberate freshness relationship supply their own times.
    for name, value in model.mtimes_ns.items():
        os.utime(directory / safe_relative(name), ns=(value, value))
    case = h.Case(directory, model.id, 'native.modules', 'required', model.args, 5,
                  model.environment, model.expected, model.output)
    return case, records


def run_campaign(selected: list[Model], oracle: Path, candidate: Path | None,
                 compiler: str, root: Path) -> dict[str, Any]:
    inputs = root / 'inputs'
    inputs.mkdir()
    runtime = root / 'runtime'
    runtime.mkdir()
    records = []
    for model in selected:
        case, compilation = prepare(model, inputs / model.id, compiler)
        observations = [h.execute(case, runtime, binary) for binary in
                        ([oracle, oracle] if candidate is None else [oracle, oracle, candidate])]
        judgment = assess(model, case, observations)
        record = {'id': model.id, 'args': list(model.args), 'environment': model.environment,
                  'expected': model.expected, 'expected_seen_hex': model.seen.hex(),
                  'input_files_hex': {name: value.hex() for name, value in model.files.items()},
                  'input_mtimes_ns': model.mtimes_ns, 'input_symlinks': model.symlinks, 'output': model.output,
                  'compilation': compilation, **judgment}
        records.append(record)
        print(('PASS ' if judgment['passed'] else 'FAIL ') + model.id, flush=True)
    return {'classification': ('Original native-module oracle model qualification; zero conformance or exception credit'
                               if candidate is None else
                               'Original native-module differential gate; ledger credit is assigned separately'),
            'oracle_only': candidate is None, 'passed': all(r['passed'] for r in records),
            'totals': {'cases': len(records), 'pass': sum(r['passed'] for r in records),
                       'fail': sum(not r['passed'] for r in records)}, 'records': records}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=ROOT / '.tools/oracle/bin/make')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--oracle-only', action='store_true')
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/native-modules/campaign.json')
    parser.add_argument('--cc', default='cc', help='compiler executable, without shell arguments')
    args = parser.parse_args(argv)
    if args.oracle_only == (args.candidate is not None):
        parser.error('supply either --candidate or --oracle-only')
    report: dict[str, Any] = {'passed': False}
    try:
        available = models()
        unknown = set(args.case) - {case.id for case in available}
        if unknown:
            raise h.HarnessError('unknown native cases: ' + ', '.join(sorted(unknown)))
        selected = [case for case in available if not args.case or case.id in args.case]
        compiler = shutil.which(args.cc)
        if compiler is None:
            raise h.HarnessError('C compiler executable not found: ' + args.cc)
        reference_oracle.verify(args.oracle)
        identities = {'oracle': h.executable_identity(args.oracle, oracle=True)}
        if args.candidate is not None:
            identities['candidate'] = h.executable_identity(args.candidate)
        with tempfile.TemporaryDirectory(prefix='rmake-native-modules-') as temporary:
            report = run_campaign(selected, Path(identities['oracle']['path']),
                                  Path(identities['candidate']['path']) if args.candidate is not None else None,
                                  compiler, Path(temporary))
        report['identities'] = identities
        report['compiler'] = compiler
        status = 0 if report['passed'] else 1
    except (h.HarnessError, reference_oracle.OracleError, OSError) as error:
        report.update(error=str(error), passed=False)
        print('ERROR ' + str(error), flush=True)
        status = 2
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print('Native module gate:', report.get('totals', {'error': True}), flush=True)
    return status


if __name__ == '__main__':
    raise SystemExit(main())
