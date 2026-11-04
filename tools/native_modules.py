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
from typing import Any

import conformance as h
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / 'tests/native'


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
    result.extend(additional_models())
    result.extend(stdio_models())
    result.extend(registration_models())
    result.extend(final_boundary_models())
    result.extend(deep_models())
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
    for name, target in model.symlinks.items():
        link = directory / safe_relative(name)
        link.parent.mkdir(parents=True, exist_ok=True)
        link.symlink_to(target)
    h.validate_fixture(directory)
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
