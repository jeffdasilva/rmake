#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Profile allocation requests on trusted, original, recipe-free core workloads.

Development-only glibc interposition. Sampled stacks are hints, not CPU timings,
complete allocator coverage, retained memory or a security sandbox.
"""
from __future__ import annotations

import argparse
import hashlib
from collections import Counter
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

import benchmark
import conformance

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'tools/profiling/allocations.c'
LIMITS = ('glibc malloc/calloc/realloc/free after helper initialization; aligned '
          'allocation entry points excluded; requested bytes are not live/peak '
          'memory; stride samples are call-stack hints, not CPU or elapsed time')


def parse_trace(text):
    lines = text.splitlines()
    if not lines or not re.fullmatch(r'summary calls=\d+ requested_bytes=\d+ frees=\d+', lines[-1]):
        raise ValueError('missing complete allocation summary')
    summary = {key: int(value) for key, value in re.findall(r'(\w+)=(\d+)', lines[-1])}
    samples = []
    for line in lines[:-1]:
        if not re.fullmatch(r'sample \d+ \d+(?: 0x[0-9a-f]+)*', line):
            raise ValueError('invalid allocation trace record')
        _, call, size, *addresses = line.split()
        samples.append({'call': int(call), 'bytes': int(size), 'addresses': addresses})
    if any(s['call'] > summary['calls'] for s in samples):
        raise ValueError('sample exceeds summary call count')
    return summary, samples


def run(candidate, workload, output):
    binary = conformance.executable_identity(candidate)
    source = dict(benchmark.workloads())[workload]
    output.mkdir(parents=True, exist_ok=True)
    helper = output / 'allocations.so'
    subprocess.run(['cc', '-std=c11', '-Wall', '-Wextra', '-Werror', '-O2', '-fPIC', '-shared',
                    str(SOURCE), '-ldl', '-o', str(helper)], check=True, capture_output=True, timeout=30)
    trace = output / 'trace.txt'
    with tempfile.TemporaryDirectory(prefix='rmake-allocation-profile-') as tmp:
        root = Path(tmp)
        (root / 'Makefile').write_bytes(source)
        alias = root / 'make'
        alias.symlink_to(binary['path'])
        (root / 'home').mkdir()
        (root / 'tmp').mkdir()
        environment = conformance.clean_environment(root, {
            'LD_PRELOAD': str(helper), 'RMAKE_ALLOC_TRACE': str(trace), 'MAKELEVEL': '0'})
        result = subprocess.run([str(alias), '-rR', '--no-print-directory'], cwd=root,
                                env=environment, capture_output=True, timeout=30, check=True)
        if result.stdout != b"make: Nothing to be done for 'all'.\n" or result.stderr:
            raise ValueError(f'profile workload produced unexpected output: {result.stdout!r} {result.stderr!r}')
    summary, samples = parse_trace(trace.read_text())
    addresses = sorted({address for sample in samples for address in sample['addresses']})
    symbols = {}
    if addresses:
        resolved = subprocess.run(['addr2line', '-Cfpie', binary['path'], *addresses], check=True,
                                  capture_output=True, text=True, timeout=30).stdout.splitlines()
        if len(resolved) != len(addresses):
            raise ValueError('incomplete address symbolization')
        symbols = dict(zip(addresses, resolved))
    counts = Counter()
    for sample in samples:
        counts.update(set(symbols[address] for address in sample['addresses']))
    report = {'workload': workload, 'source_sha256': hashlib.sha256(source).hexdigest(),
              'binary': binary, 'helper_sha256': conformance.sha256_file(SOURCE), 'limits': LIMITS,
              'summary': summary, 'samples': samples, 'symbols': symbols,
              'sampled_callers': [{'symbol': name, 'samples': count} for name, count in counts.most_common()]}
    (output / 'report.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(json.dumps(summary, sort_keys=True))
    for name, count in counts.most_common(15):
        print(f'{count:5} {name}')
    print(f'Report: {output / "report.json"}')
    return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--workload', choices=('tiny-noop', 'large-noop', 'expansion'), default='large-noop')
    parser.add_argument('--output', type=Path, default=ROOT / 'artifacts/profile/allocations')
    args = parser.parse_args(argv)
    try:
        run(args.candidate.expanduser().resolve(), args.workload, args.output.expanduser().resolve())
        return 0
    except (OSError, ValueError, subprocess.SubprocessError, conformance.HarnessError) as error:
        print(f'allocation profile: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
