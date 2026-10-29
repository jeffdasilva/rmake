#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original no-op/expansion/parallel workloads; optional paired, output-checked comparison."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import time

import conformance
import oracle as reference_oracle

ROOT = Path(__file__).resolve().parent.parent


def workloads():
    yield "tiny-noop", b"all: ;\n"
    names = [f"node{i}" for i in range(10000)]
    yield "large-noop", (
        "all: " + " ".join(names) + "\n" + "".join(f"{name}: ;\n" for name in names)
    ).encode()
    yield "expansion", (
        "WORD := alpha beta gamma\n"
        + "".join(f"V{i} := $(subst beta,B,$(WORD))\n" for i in range(5000))
        + "all: ;\n"
    ).encode()


def startup_workloads():
    """Large explicit argv, with every assignment value checked in raw output."""
    for count in (0, 1000, 5000):
        references = b"|".join(f"$(ARG_{index})".encode() for index in range(count))
        yield f"startup-arguments-{count}", (
            b"$(info values=[" + references + b"])\n.PHONY: all\nall: ;\n")


def eval_workloads():
    """Observe every separately evaluated binding, without recipe children."""
    for count in (0, 1000):
        references = b"|".join(f"$(EV_{index})".encode() for index in range(count))
        yield f"eval-arguments-{count}", b"$(info values=[" + references + b"])\nall: ;\n"


def startup_effect_workloads():
    """Observe all eager CLI effects while measuring optional replay capture."""
    for count in (0, 1000):
        references = b"|".join(f"$(SIDE_{index})".encode() for index in range(count))
        yield f"startup-effects-{count}", b"$(info values=[" + references + b"])\nall: ;\n"


def initial_flag_workloads():
    """Observe raw dollar decoding and every eager inherited assignment effect."""
    for count in (0, 1000):
        references = b"|".join(f"$(IV_{index})".encode() for index in range(count))
        yield f"initial-flags-{count}", b"$(info sentinel=[$(value FLAG)])\n$(info values=[" + references + b"])\nall: ;\n"


def runtime_include_workloads():
    """Observe every metadata construction with no recipe or shell children."""
    for count in (0, 1000):
        yield f'runtime-includes-{count}', (b'MAKEFLAGS += -s\n' * count
            + f'$(info COUNT=[{count}] S=[$(filter rRs,$(firstword $(MAKEFLAGS)))])\nall: ;\n'.encode())


def workload_environment(name):
    if name.startswith('initial-flags-'):
        count = int(name.rsplit('-', 1)[1])
        payload = 'rR FLAG=$$'
        payload += ''.join(f' IF_{i}:=$$(eval\\ IV_{i}:=value{i})' for i in range(count))
        return {'GNUMAKEFLAGS': payload}
    return {}


def builtin_transition_workloads():
    """Late -R with unused user declarations and observed cleanup semantics."""
    for count in (0, 1000, 10000):
        yield f'builtin-transitions-{count}', (
            b'$(info before=$(origin CC)/$(origin .LIBPATTERNS))\n'
            b'MAKEFLAGS += -R\n.PHONY: all\n'
            b'all:;@$(info after=$(origin CC)/$(origin .LIBPATTERNS)/$(origin .SHELLFLAGS)/$(words $(SUFFIXES))):\n'
            + b''.join(f'unused{index}:;\n'.encode() for index in range(count)))


def span_workloads():
    """Fixed large-list costs, including early exits and required full scans."""
    prelude = b"TEXT := " + b"one  two\t" * 100000 + b"\n"
    expressions = (
        ("wordlist-prefix", b"$(wordlist 1,1,$(TEXT))"),
        ("wordlist-middle", b"$(wordlist 100000,100002,$(TEXT))"),
        ("wordlist-full", b"$(wordlist 1,9223372036854775807,$(TEXT))"),
        ("wordlist-outside", b"$(wordlist 9223372036854775807,9223372036854775807,$(TEXT))"),
        ("let-remainder", b"$(let first rest,$(TEXT),$(lastword $(value rest)))"),
        ("patsubst-literal", b"$(patsubst one,replaced,$(TEXT))"),
    )
    for name, expression in expressions:
        yield name, (prelude + b"OUT := " + expression + b"\n"
                     b"$(info [$(words $(OUT))|$(firstword $(OUT))|$(lastword $(OUT))])\n"
                     b"all: ;\n")


def selected_workloads(suite):
    if suite in ("core", "all"):
        yield from workloads()
    if suite in ("startup-arguments", "all"):
        yield from startup_workloads()
    if suite in ("eval-arguments", "all"):
        yield from eval_workloads()
    if suite in ("startup-effects", "all"):
        yield from startup_effect_workloads()
    if suite in ("initial-flags", "all"):
        yield from initial_flag_workloads()
    if suite in ("runtime-includes", "all"):
        yield from runtime_include_workloads()
    if suite in ("builtin-transitions", "all"):
        yield from builtin_transition_workloads()
    if suite in ("word-spans", "all"):
        yield from span_workloads()
    if suite in ("wildcard", "all"):
        yield from wildcard_workloads()
    if suite in ("bindings", "all"):
        yield from binding_workloads()
    if suite in ("implicit-search", "all"):
        yield from implicit_search_workloads()
    if suite in ("parallel-branches", "all"):
        yield from parallel_branch_workloads()
    if suite in ("target-contexts", "all"):
        yield from target_context_workloads()
    if suite in ("archives", "all"):
        yield from archive_workloads()
    if suite in ("archive-sequences", "all"):
        yield from archive_sequence_workloads()
    if suite in ("vpath-search", "all"):
        yield from vpath_workloads()
    if suite in ("secondary", "all"):
        yield from secondary_workloads()
    if suite in ("implicit-rejections", "all"):
        yield from implicit_rejection_workloads()
    if suite in ("gpath", "all"):
        yield from gpath_workloads()
    if suite in ("intermediates", "all"):
        yield from intermediate_workloads()
    if suite in ("lazy-records", "all"):
        yield from lazy_record_workloads()
    if suite in ("vpath-records", "all"):
        yield from vpath_record_workloads()
    if suite in ("directory-search", "all"):
        yield from directory_search_workloads()
    if suite in ("archive-names", "all"):
        yield from archive_name_workloads()
    if suite in ("symlink-times", "all"):
        yield from symlink_time_workloads()
    if suite in ("rule-filenames", "all"):
        yield from rule_filename_workloads()
    if suite in ("rule-tilde", "all"):
        yield from rule_tilde_workloads()
    if suite in ("libraries", "all"):
        yield from library_workloads()


def library_workloads():
    """No-child library lookup, including cross-pattern directory priority."""
    for kind, count in (("local", 512), ("remote", 512), ("ranked", 256)):
        patterns = b'bench-library-%.so bench-library-%.a' if kind == 'ranked' else b'bench-library-%.a'
        path = b'' if kind == 'local' else b'VPATH=library-first' + (b':library-second' if kind == 'ranked' else b'') + b'\n'
        source = b'.LIBPATTERNS=' + patterns + b'\n' + path
        source += ('all: ' + ' '.join(f'-l{index:04d}' for index in range(count)) + ' ;\n').encode()
        yield f'libraries-{kind}-{count}', source


def rule_tilde_workloads():
    """Global HOME at read/search time, fixed existing inputs and no children."""
    prelude = b'unexport HOME\nHOME=tilde-inputs\n'
    names = ' '.join(f'~/file{index:04d}.in' for index in range(2000))
    yield 'rule-tilde-literal-2000', prelude + ('all: ' + names + ' ;\n').encode()
    yield 'rule-tilde-glob-1000', prelude + b'all: ~/*.in ;\n'
    yield 'rule-tilde-secondary-1000', (prelude + b'.SECONDEXPANSION:\nP=~/*.in\nall: $$(P) ;\n')
    targets = ' '.join(f'file{index:04d}.out' for index in range(500))
    yield 'rule-tilde-implicit-500', prelude + ('all: ' + targets + ' ;\n%.out: ~/%.in ;\n').encode()


def rule_filename_workloads():
    """Parsing and filename identity costs with no recipe subprocesses."""
    quoted = [f'rule\\ {index:04d}' for index in range(2000)]
    yield 'rule-filenames-quoted-2000', ('all: ' + ' '.join(quoted) + ' ;\n'
                                        + ''.join(name + ': ;\n' for name in quoted)).encode()
    yield 'rule-filenames-glob-1000', b'all: rule-inputs/*.in ;\n'
    yield 'rule-filenames-secondary-1000', (b'.SECONDEXPANSION:\nP=rule-inputs/*.in\n'
                                           b'all: $$(P) ;\n')
    targets = ' '.join(f'rule\\ {index:04d}.out' for index in range(500))
    yield 'rule-filenames-implicit-500', ('all: ' + targets + ' ;\n%.out: %.in ;\n').encode()


def symlink_time_workloads():
    """Existing fixed-time outputs; link-aware checks launch no children."""
    yield 'symlink-times-plain-2000', (
        'all: ' + ' '.join(f'sym-plain-{index:04d}' for index in range(2000)) + ' ;\n').encode()
    for kind, count in (('links', 1000), ('chain', 500), ('dangling', 1000)):
        source = 'all: ' + ' '.join(f'sym-{index:04d}.out' for index in range(count)) + ' ;\n'
        source += ''.join(f'sym-{index:04d}.out: sym-links/in-{index:04d} ;\n' for index in range(count))
        yield f'symlink-times-{kind}-{count}', source.encode()


def archive_name_workloads():
    """Canonical long names accepted by both old and current scanners."""
    for kind in ('gnu', 'bsd'):
        members = ' '.join(f'lib.a(long-archive-member-{index:04d}.o)' for index in range(128))
        yield f'archive-names-{kind}-128', f'all: {members}\n'.encode()
    yield 'archive-names-glob-256', (
        ''.join(f'V{index} := $(words $(wildcard lib.a(*.o)))\n' for index in range(64))
        + '$(info $(V63))\nall: ;\n').encode()


def directory_search_workloads():
    """Cached implicit membership and repeated negative VPATH probes."""
    outputs = ' '.join(f'cache-node{index:04d}.out' for index in range(1000))
    yield 'directory-search-implicit-1000', (
        'all: ' + outputs + ' ;\n%.out: inputs/%.in ;\n').encode()
    missing = ' '.join(f'cache-missing{index:04d}.dat' for index in range(1000))
    yield 'directory-search-vpath-misses-1000', (
        'VPATH=' + ':'.join('d' + str(index) for index in range(8))
        + '\nall: ' + missing + ' ;\n.DEFAULT: ;\n').encode()


def vpath_record_workloads():
    """Physical current inputs with declared searched records and no children."""
    for kind, count in (('explicit', 1000), ('aliases', 1000), ('implicit', 400)):
        names = [f'search-node{index:04d}' for index in range(count)]
        targets = [name + '.out' if kind == 'implicit' else name + '.in' for name in names]
        if kind == 'aliases':
            targets = [target for name in names for target in (name + '.in', 'records/' + name + '.in')]
        source = 'VPATH = records\nall: ' + ' '.join(targets) + ' ;\n'
        source += ''.join('records/' + name + '.in: ;\n' for name in names)
        if kind == 'implicit':
            source += '%.out: %.in ;\n'
        yield f'vpath-record-{kind}-{count}', source.encode()


def lazy_record_workloads():
    """Grouped and first-double-colon records beneath current real outputs."""
    names = [f'record-node{index:04d}' for index in range(1000)]
    for kind in ('grouped', 'double'):
        source = '.SECONDARY:\n.SECONDEXPANSION:\nall: ' + ' '.join(name + '.out' for name in names) + ' ;\n'
        for name in names:
            source += name + '.out: ' + name + '.mid ;\n'
            if kind == 'grouped':
                source += name + '.mid ' + name + '.peer &: $$(strip) record-input ;\n'
            else:
                source += name + '.mid:: $$(strip) record-input ;\n'
                source += name + '.mid:: record-input ;\n'
        yield 'lazy-record-' + kind + '-1000', source.encode()


def intermediate_workloads():
    """Current outputs with missing middles; literal empty recipes launch no children."""
    for policy in ('secondary', 'not-intermediate'):
        names = [f'lazy-node{index:04d}.out' for index in range(1000)]
        middles = [name.removesuffix('.out') + '.mid' for name in names]
        prelude = '.SECONDARY:\n' if policy == 'secondary' else '.NOTINTERMEDIATE:\n.INTERMEDIATE: ' + ' '.join(middles) + '\n'
        source = prelude + 'all: ' + ' '.join(names) + ' ;\n'
        for name, middle in zip(names, middles):
            source += name + ': ' + middle + ' ;\n' + middle + ': lazy-input ;\n'
        yield 'intermediate-' + policy + '-1000', source.encode()


def gpath_workloads():
    """Permanent path lookup and alias reuse, with real no-op input files."""
    names = [f'gpath-node{index:04d}.out' for index in range(1000)]
    prelude = 'VPATH = inputs\nGPATH = inputs\n'
    for aliases in (False, True):
        goals = names + (['inputs/' + name for name in names] if aliases else [])
        source = prelude + 'all: ' + ' '.join(goals) + ' ;\n'
        source += ''.join(name + ': ;\n' for name in names)
        yield 'gpath-aliases-1000' if aliases else 'gpath-explicit-1000', source.encode()
    yield 'gpath-implicit-400', (prelude + 'all: ' + ' '.join(names[:400])
                                + ' ;\n%.out: %.in ;\n').encode()
    ordinary = [f'ordinary-node{index:04d}.src' for index in range(1000)]
    yield 'gpath-mixed-1000', (
        'VPATH = inputs other\nGPATH = inputs\nall: ' + ' '.join(names + ordinary)
        + '\n\t@$(if $(filter 2000,$(words $^)),,$(error missing prerequisites))'
        + '$(if $(filter 1000,$(words $(filter inputs/%,$^))),,$(error missing permanent paths))'
        + '$(if $(filter 1000,$(words $(filter other/%,$^))),,$(error missing tentative paths))\n'
        + ''.join(name + ': ;\n' for name in names)).encode()


def implicit_rejection_workloads():
    """Pure rejected searches: explicit empty targets keep the invocation valid."""
    names = ' '.join(f'rejected{index}.out' for index in range(64))
    for count in (64, 256):
        source = 'all: ' + names + '\n' + names + ':\n'
        source += ''.join(f'%.out: absent{index};\n' for index in range(count))
        yield f'implicit-rejected-{count}', source.encode()


def secondary_workloads():
    """Ready lists and many small deferred clauses; no process launches."""
    for count in (256, 1024):
        names = ' '.join(f'secondary{index}' for index in range(count))
        yield f'secondary-ready-{count}', (
            '.SECONDEXPANSION:\nall: ' + names + '\n'
            'all: $$(if $$(filter secondary0,$$^),,$$(error missing ready prerequisite)) ;\n'
            + names + ': ;\n').encode()
    yield 'secondary-empty-256', (
        b'.SECONDEXPANSION:\n' + b'all: $$(strip)\n' * 256 + b'all: ;\n')
    names = ' '.join(f'secondary{index}' for index in range(1000))
    yield 'secondary-targets-1000', (
        '.SECONDEXPANSION:\nall: ' + names + '\n' + names + ': $$(strip) ;\n').encode()


def vpath_workloads():
    """Repeated pure pattern selection plus fresh VPATH filesystem lookups."""
    targets = ' '.join(f'vpath-node{index:04d}.src' for index in range(1000))
    for count in (64, 256):
        rules = ''.join(f'vpath ignored{index}%.unused inputs\n' for index in range(count))
        yield f'vpath-unmatched-{count}', (rules + 'vpath %.src inputs\nall: ' + targets + '\n').encode()
    yield 'vpath-first-hit-64', ('vpath %.src inputs\n' * 64 + 'all: ' + targets + '\n').encode()
    members = ' '.join(f'vpath-lib.a(member{index:04d}.o)' for index in range(64))
    rules = ''.join(f'vpath ignored{index}%.unused inputs\n' for index in range(64))
    yield 'vpath-archive-64', (rules + 'vpath %.a inputs\nall: ' + members + '\n').encode()


def archive_sequence_workloads():
    """Unclosed groups stress suffix recognition without matching any files."""
    for count in (4096, 16384):
        yield f'archive-sequence-unclosed-{count}', (
            b'TEXT := ' + b'absent.a(member ' * count
            + b'\n$(info $(words $(wildcard $(TEXT))))\nall:;\n')


def archive_workloads():
    """Existing members with fixed historical dates and no child launches."""
    for count in (64, 256):
        members = ' '.join(f'lib.a(member{index:04d}.o)' for index in range(count))
        yield f'archive-noop-{count}', f'all: {members}\n'.encode()
    # Real object payloads can make read-ahead more expensive than header-only
    # seeks. Keep these separate from the dense one-byte member workloads.
    for payload in (4096, 65536):
        members = ' '.join(f'lib.a(member{index:04d}.o)' for index in range(64))
        yield f'archive-payload-64-{payload}', f'all: {members}\n'.encode()


def target_context_workloads():
    """Activate variable scopes and automatic values without launching children."""
    names = [f"node{index}" for index in range(3000)]
    for scope in ("none", "explicit", "pattern"):
        source = "all: " + " ".join(names) + "\n"
        if scope == "pattern":
            source += "node%: VALUE = scoped\n"
        for name in names:
            if scope == "explicit":
                source += name + ": VALUE = scoped\n"
            source += name + ":;@$(if $(filter node%,$@),,$(error missing automatic target))"
            if scope != "none":
                source += "$(if $(filter scoped,$(VALUE)),,$(error missing scoped value))"
            source += "$(if $(filter /%,$(abspath input)),,$(error missing directory))\n"
        yield "target-context-" + scope, source.encode()


def parallel_branch_workloads():
    """Nested independent branches, rebuilt every sample without file outputs."""
    for depth in (1, 4):
        branches = [f"branch{i}_0" for i in range(16)]
        source = "all: " + " ".join(branches) + "\n"
        for index in range(16):
            for level in range(depth):
                source += f"branch{index}_{level}: branch{index}_{level+1}\n"
            source += f"branch{index}_{depth}:;@sleep .02\n"
        yield f'parallel-branches-depth-{depth}', source.encode()


def workload_arguments(name):
    arguments = (["--no-print-directory"] if name.startswith('builtin-transitions-')
                 else ["-rR", "--no-print-directory"])
    if name.startswith('runtime-includes-'):
        arguments += ['-I-', '-Icounter-$(info tick)']
    if name.startswith('startup-arguments-'):
        count = int(name.removeprefix('startup-arguments-'))
        arguments.extend(f"ARG_{index}=value{index}" for index in range(count))
    if name.startswith('startup-effects-'):
        count = int(name.removeprefix('startup-effects-'))
        arguments.extend(f"CHECK_{index}:=$(eval SIDE_{index}:=value{index})" for index in range(count))
    if name.startswith('eval-arguments-'):
        count = int(name.removeprefix('eval-arguments-'))
        arguments.extend(f"--eval=EV_{index}=value{index}" for index in range(count))
    if name.startswith('symlink-times-'):
        arguments.append('-L')
    if name.startswith('parallel-branches-'):
        arguments += ["-j4", "--jobserver-style=pipe"]
    return arguments


def implicit_search_workloads():
    """Many irrelevant definitions, with real resolution but no child launches."""
    for count in (64, 256):
        targets = ' '.join(f'node{i}.out' for i in range(400))
        patterns = ''.join(f'ignored{i}%.unused{i}:;\n' for i in range(count))
        yield f'implicit-unmatched-{count}', (
            f'all: {targets}\n' + patterns + '%.out:;\n').encode()


def binding_workloads():
    """Repeated reads expose value-copy costs that a single lookup can hide."""
    for size in (262144, 1048576):
        for local in (False, True):
            refs = b"$(X)" * 8 if local else b"$(TEXT)" * 8
            expression = b"$(foreach X,$(TEXT)," + refs + b")" if local else refs
            source = (b"TEXT := " + b"x" * size + b"\n"
                      + (b"OUT := " + expression + b"\n") * 5
                      + b"$(info $(words $(OUT)))\nall:;@:\n")
            yield f"simple-{'local' if local else 'global'}-{size}", source


def wildcard_workloads():
    for name, pattern, repeats in (
        ('wildcard-cold', 'inputs/*.c', 1),
        ('wildcard-warm', 'inputs/*.c', 200),
        ('wildcard-complex', 'inputs/file[0-9][0-9][0-9][0-9].c', 200),
        ('wildcard-literal', ' '.join(f'inputs/file{i:04d}.c' for i in range(1000)), 1),
    ):
        source = ''.join(f'V{i} := $(words $(wildcard {pattern}))\n' for i in range(repeats))
        yield name, (source + f'$(info $(V{repeats-1}))\nall: ;\n').encode()


def prepare_archive(path, count, payload):
    data = bytearray(b'!<arch>\n')
    for index in range(count):
        fields = (f'member{index:04d}.o/'.encode(), b'1600000000', b'0', b'0', b'644', str(payload).encode())
        data.extend(b''.join(value.ljust(width, b' ') for value, width in
                             zip(fields, (16, 12, 6, 6, 8, 10))))
        data.extend(b'`\n' + b'x' * payload + (b'\n' if payload % 2 else b''))
    path.write_bytes(data)
    return {'members': count, 'member_bytes': payload,
            'archive_sha256': hashlib.sha256(data).hexdigest()}


def prepare_archive_names(path, kind, count):
    names = [f'long-archive-member-{index:04d}.o'.encode() for index in range(count)]
    data = bytearray(b'!<arch>\n')

    def append(name, payload):
        fields = (name, b'1600000000', b'0', b'0', b'644', str(len(payload)).encode())
        assert all(len(value) <= width for value, width in zip(fields, (16, 12, 6, 6, 8, 10)))
        data.extend(b''.join(value.ljust(width, b' ') for value, width in
                             zip(fields, (16, 12, 6, 6, 8, 10))))
        data.extend(b'`\n' + payload + (b'\n' if len(payload) % 2 else b''))

    if kind == 'gnu':
        append(b'//', b''.join(name + b'/\n' for name in names))
        offset = 0
        for name in names:
            append(b'/' + str(offset).encode(), b'x')
            offset += len(name) + 2
    else:
        for name in names:
            append(b'#1/' + str(len(name)).encode(), name + b'x')
    path.write_bytes(data)
    return {'members': count, 'format': kind,
            'archive_sha256': hashlib.sha256(data).hexdigest()}


def prepare_files(path, workload):
    if workload.startswith('runtime-includes-'):
        (path / 'counter-$(info tick)').mkdir(exist_ok=True)
        return {'metadata_constructions': int(workload.rsplit('-', 1)[1]) + 1,
                'recipe_subprocesses': 0, 'physical_directories': 1}
    if workload.startswith('libraries-'):
        _, kind, count = workload.split('-')
        count = int(count)
        for previous in path.glob('bench-library-*'):
            previous.unlink()
        directories = [path / 'library-first', path / 'library-second']
        for directory in directories:
            directory.mkdir(exist_ok=True)
            for previous in directory.iterdir():
                previous.unlink()
        locations = [(path if kind == 'local' else directories[0], '.a')]
        if kind == 'ranked':
            locations.append((directories[1], '.so'))
        for directory, suffix in locations:
            for index in range(count):
                target = directory / f'bench-library-{index:04d}{suffix}'
                target.write_bytes(b'input\n')
                os.utime(target, ns=(1600000000000000000,) * 2)
        return {'libraries': count, 'files': count * len(locations), 'kind': kind,
                'recipe_subprocesses': 0}
    if workload.startswith('rule-tilde-'):
        count = int(workload.rsplit('-', 1)[1])
        directory = path / 'tilde-inputs'
        directory.mkdir(exist_ok=True)
        for previous in directory.iterdir():
            previous.unlink()
        for index in range(count):
            target = directory / f'file{index:04d}.in'
            target.write_bytes(b'input\n')
            os.utime(target, ns=(1600000000000000000,) * 2)
        return {'files': count, 'home': 'tilde-inputs', 'recipe_subprocesses': 0,
                'content_sha256': hashlib.sha256(b'input\n').hexdigest()}
    if workload.startswith('rule-filenames-'):
        kind, count = workload.removeprefix('rule-filenames-').rsplit('-', 1)
        count = int(count)
        directory = path / 'rule-inputs'
        directory.mkdir(exist_ok=True)
        for previous in directory.iterdir():
            previous.unlink()
        for previous in path.glob('rule *.in'):
            previous.unlink()
        if kind in ('glob', 'secondary', 'implicit'):
            for index in range(count):
                target = (path / f'rule {index:04d}.in' if kind == 'implicit'
                          else directory / f'input {index:04d}.in')
                target.write_bytes(b'input\n')
                os.utime(target, ns=(1600000000000000000,) * 2)
        return {'files': 0 if kind == 'quoted' else count,
                'quoted_names': count, 'recipe_subprocesses': 0}
    if workload.startswith('symlink-times-'):
        _, _, kind, count = workload.split('-')
        count = int(count)
        # Each workload owns this namespace inside a disposable campaign root.
        # Clear previous live links so dangling and chain controls stay distinct.
        for pattern in ('sym-*.out', 'sym-plain-*'):
            for previous in path.glob(pattern):
                previous.unlink()
        for directory in (path / 'sym-links', path / 'sym-inputs'):
            if directory.is_dir():
                for previous in directory.iterdir():
                    previous.unlink()
        if kind == 'plain':
            for index in range(count):
                target = path / f'sym-plain-{index:04d}'
                target.write_bytes(b'original\n')
                os.utime(target, ns=(1600000000000000000,) * 2)
            return {'regular_files': count, 'links': 0, 'mtime_ns': 1600000000000000000}
        links = path / 'sym-links'
        inputs = path / 'sym-inputs'
        links.mkdir(exist_ok=True)
        inputs.mkdir(exist_ok=True)
        for index in range(count):
            output = path / f'sym-{index:04d}.out'
            output.write_bytes(b'original\n')
            os.utime(output, ns=(1600000020000000000,) * 2)
            link = links / f'in-{index:04d}'
            link.unlink(missing_ok=True)
            if kind == 'chain':
                hop = links / f'hop-{index:04d}'
                hop.unlink(missing_ok=True)
                hop.symlink_to(f'../sym-inputs/in-{index:04d}')
                os.utime(hop, ns=(1600000015000000000,) * 2, follow_symlinks=False)
                link.symlink_to(hop.name)
            else:
                link.symlink_to(f'../sym-inputs/in-{index:04d}')
            os.utime(link, ns=(1600000010000000000,) * 2, follow_symlinks=False)
            leaf = inputs / link.name
            if kind == 'dangling':
                leaf.unlink(missing_ok=True)
            else:
                leaf.write_bytes(b'original\n')
                os.utime(leaf, ns=(1600000000000000000,) * 2)
        return {'outputs': count, 'links': count * (2 if kind == 'chain' else 1),
                'referents': 0 if kind == 'dangling' else count,
                'output_mtime_ns': 1600000020000000000,
                'link_mtime_ns': 1600000010000000000,
                'hop_mtime_ns': 1600000015000000000 if kind == 'chain' else None}
    if workload.startswith('archive-names-'):
        _, _, kind, count = workload.split('-')
        return prepare_archive_names(path / 'lib.a', 'gnu' if kind == 'glob' else kind, int(count))
    if workload.startswith('directory-search-'):
        files = []
        if workload == 'directory-search-implicit-1000':
            (path / 'inputs').mkdir(exist_ok=True)
            for index in range(1000):
                files.extend((path / f'cache-node{index:04d}.out',
                              path / 'inputs' / f'cache-node{index:04d}.in'))
        else:
            for index in range(8):
                directory = path / ('d' + str(index))
                directory.mkdir(exist_ok=True)
                files.extend(directory / f'unused{item:04d}.in' for item in range(64))
        for filename in files:
            filename.write_bytes(b'original\n')
            os.utime(filename, ns=(1600000000000000000, 1600000000000000000))
        return {'files': len(files), 'mtime_ns': 1600000000000000000,
                'content_sha256': hashlib.sha256(b'original\n').hexdigest()}
    if workload.startswith('vpath-record-'):
        directory = path / 'records'
        directory.mkdir(exist_ok=True)
        count = 400 if workload == 'vpath-record-implicit-400' else 1000
        for index in range(count):
            files = [directory / f'search-node{index:04d}.in']
            if workload == 'vpath-record-implicit-400':
                files.append(path / f'search-node{index:04d}.out')
            for filename in files:
                filename.write_bytes(b'original\n')
                os.utime(filename, ns=(1600000000000000000, 1600000000000000000))
        return {'inputs': count, 'outputs': count if workload == 'vpath-record-implicit-400' else 0,
                'mtime_ns': 1600000000000000000,
                'content_sha256': hashlib.sha256(b'original\n').hexdigest()}
    if workload.startswith('lazy-record-'):
        source = path / 'record-input'
        source.write_bytes(b'original\n')
        os.utime(source, ns=(1600000000000000000, 1600000000000000000))
        for index in range(1000):
            output = path / f'record-node{index:04d}.out'
            output.write_bytes(b'original\n')
            os.utime(output, ns=(1600000002000000000, 1600000002000000000))
        return {'outputs': 1000, 'input_mtime_ns': 1600000000000000000,
                'output_mtime_ns': 1600000002000000000,
                'content_sha256': hashlib.sha256(b'original\n').hexdigest()}
    if workload.startswith('intermediate-'):
        source = path / 'lazy-input'
        source.write_bytes(b'original\n')
        os.utime(source, ns=(1600000000000000000, 1600000000000000000))
        for index in range(1000):
            output = path / f'lazy-node{index:04d}.out'
            output.write_bytes(b'original\n')
            os.utime(output, ns=(1600000002000000000, 1600000002000000000))
        return {'outputs': 1000, 'input_mtime_ns': 1600000000000000000,
                'output_mtime_ns': 1600000002000000000,
                'content_sha256': hashlib.sha256(b'original\n').hexdigest()}
    if workload.startswith(('archive-noop-', 'archive-payload-')):
        if workload.startswith('archive-noop-'):
            count, payload = int(workload.removeprefix('archive-noop-')), 1
        else:
            count, payload = map(int, workload.removeprefix('archive-payload-').split('-'))
        return prepare_archive(path / 'lib.a', count, payload)
    if workload.startswith('gpath-'):
        inputs = path / 'inputs'
        inputs.mkdir(exist_ok=True)
        count = 400 if workload == 'gpath-implicit-400' else 1000
        for index in range(count):
            for extension in ('out', 'in'):
                filename = inputs / f'gpath-node{index:04d}.{extension}'
                filename.write_bytes(b'original\n')
                os.utime(filename, ns=(1600000000000000000, 1600000000000000000))
        extra = 0
        if workload == 'gpath-mixed-1000':
            other = path / 'other'
            other.mkdir(exist_ok=True)
            extra = 1000
            for index in range(extra):
                filename = other / f'ordinary-node{index:04d}.src'
                filename.write_bytes(b'original\n')
                os.utime(filename, ns=(1600000000000000000, 1600000000000000000))
        return {'files': count * 2 + extra, 'mtime_ns': 1600000000000000000,
                'content_sha256': hashlib.sha256(b'original\n').hexdigest()}
    if workload.startswith('vpath-'):
        inputs = path / 'inputs'
        inputs.mkdir(exist_ok=True)
        if workload == 'vpath-archive-64':
            return prepare_archive(inputs / 'vpath-lib.a', 64, 1)
        for index in range(1000):
            (inputs / f'vpath-node{index:04d}.src').write_bytes(b'original\n')
        return {'files': 1000, 'content_sha256': hashlib.sha256(b'original\n').hexdigest()}
    if not workload.startswith('wildcard-'):
        return None
    inputs = path / 'inputs'
    inputs.mkdir(exist_ok=True)
    for index in range(1000):
        (inputs / f'file{index:04d}.c').write_bytes(b'original\n')
    return {'files': 1000, 'pattern': 'inputs/file0000.c through inputs/file0999.c',
            'content_sha256': hashlib.sha256(b'original\n').hexdigest()}


def paired_order(iteration, candidate, baseline=False):
    engines = ["oracle", "candidate"] if candidate else ["oracle"]
    if baseline:
        engines.insert(1, "baseline")
    return engines if iteration % 2 == 0 else list(reversed(engines))


def summary(samples):
    return {"samples_ns": samples, "median_ns": statistics.median(samples),
            "min_ns": min(samples), "max_ns": max(samples)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--baseline", type=Path, help="optional previous candidate for paired before/after timing")
    parser.add_argument("--repeat", type=int, default=15)
    parser.add_argument("--suite", choices=("core", "startup-arguments", "eval-arguments", "startup-effects", "initial-flags", "runtime-includes", "builtin-transitions", "word-spans", "wildcard", "bindings", "implicit-search", "parallel-branches", "target-contexts", "archives", "archive-sequences", "vpath-search", "secondary", "implicit-rejections", "gpath", "intermediates", "lazy-records", "vpath-records", "directory-search", "archive-names", "symlink-times", "rule-filenames", "rule-tilde", "libraries", "all"), default="core")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.repeat < 3 or args.repeat > 1000:
        parser.error("--repeat must be between 3 and 1000")
    if args.baseline and not args.candidate:
        parser.error("--baseline requires --candidate")
    oracle = args.oracle.resolve()
    profile = reference_oracle.verify(oracle)
    identities = {"oracle": conformance.executable_identity(oracle, oracle=True)}
    identities["oracle"]["profile"] = profile
    executables = {"oracle": oracle}
    if args.candidate:
        identities["candidate"] = conformance.executable_identity(args.candidate)
        executables["candidate"] = Path(identities["candidate"]["path"])
    if args.baseline:
        identities["baseline"] = conformance.executable_identity(args.baseline)
        executables["baseline"] = Path(identities["baseline"]["path"])
    report = {
        "schema": 2,
        "suite": args.suite,
        "identities": identities,
        "platform": platform.platform(),
        "tool": "wall time including startup; two warmups per engine; OS page cache warm",
        "method": "alternating engine order; same path, environment, arguments and exact output",
        "limitations": ["no CPU pinning or isolation", "no CPU/RSS/I/O measurements",
                        "no confidence intervals or performance acceptance thresholds",
                        "selected early workloads, not a representative full-compatibility benchmark"],
        "workloads": [],
    }
    with tempfile.TemporaryDirectory(prefix="rmake-bench-") as directory:
        path = Path(directory)
        env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "TZ": "UTC", "HOME": directory}
        alias = path / "make"
        for name, source in selected_workloads(args.suite):
            fixture = prepare_files(path, name)
            (path / "Makefile").write_bytes(source)
            arguments = workload_arguments(name)
            overrides = workload_environment(name)
            run_environment = env | overrides
            samples = {label: [] for label in executables}
            expected = None
            for iteration in range(args.repeat + 2):
                for label in paired_order(iteration, args.candidate is not None, args.baseline is not None):
                    alias.unlink(missing_ok=True)
                    alias.symlink_to(executables[label])
                    start = time.perf_counter_ns()
                    result = subprocess.run(
                        [str(alias), *arguments], cwd=path, env=run_environment,
                        capture_output=True, timeout=30, check=True,
                    )
                    elapsed = time.perf_counter_ns() - start
                    output = (result.stdout, result.stderr)
                    if expected is not None and expected != output:
                        raise RuntimeError(f"benchmark output disagreement: {name} ({label})")
                    expected = output
                    if iteration >= 2:
                        samples[label].append(elapsed)
            record = {
                "name": name, "source_sha256": hashlib.sha256(source).hexdigest(),
                "arguments": arguments,
                "input_fixture": fixture,
                "measurements": {label: summary(values) for label, values in samples.items()},
            }
            if overrides:
                record["environment"] = overrides
            if args.candidate:
                record["paired_oracle_over_candidate"] = [
                    left / right for left, right in zip(samples["oracle"], samples["candidate"])]
                record["median_paired_ratio"] = statistics.median(record["paired_oracle_over_candidate"])
            if args.baseline:
                record["paired_baseline_over_candidate"] = [
                    left / right for left, right in zip(samples["baseline"], samples["candidate"])]
                record["median_paired_baseline_ratio"] = statistics.median(record["paired_baseline_over_candidate"])
            report["workloads"].append(record)
            display = ", ".join(f"{label} median {values['median_ns'] / 1_000_000:.3f} ms"
                                for label, values in record["measurements"].items())
            print(f"{name}: {display}")
    output_path = args.output or ROOT / f"artifacts/bench/{'comparison' if args.candidate else 'oracle'}.json"
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Report: {output_path}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError, reference_oracle.OracleError,
            conformance.HarnessError) as error:
        print(f"benchmark: {error}", file=sys.stderr)
        sys.exit(2)
