#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Original no-op/expansion/parallel workloads; optional paired, output-checked comparison."""

import argparse
import hashlib
import json
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
    arguments = ["-rR", "--no-print-directory"]
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


def prepare_files(path, workload):
    if workload.startswith(('archive-noop-', 'archive-payload-')):
        if workload.startswith('archive-noop-'):
            count, payload = int(workload.removeprefix('archive-noop-')), 1
        else:
            count, payload = map(int, workload.removeprefix('archive-payload-').split('-'))
        return prepare_archive(path / 'lib.a', count, payload)
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
    parser.add_argument("--suite", choices=("core", "word-spans", "wildcard", "bindings", "implicit-search", "parallel-branches", "target-contexts", "archives", "archive-sequences", "vpath-search", "secondary", "all"), default="core")
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
            samples = {label: [] for label in executables}
            expected = None
            for iteration in range(args.repeat + 2):
                for label in paired_order(iteration, args.candidate is not None, args.baseline is not None):
                    alias.unlink(missing_ok=True)
                    alias.symlink_to(executables[label])
                    start = time.perf_counter_ns()
                    result = subprocess.run(
                        [str(alias), *arguments], cwd=path, env=env,
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
