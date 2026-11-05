#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exact deterministic GNU2 implicit search and intermediate debug output."""
from __future__ import annotations

import argparse
from dataclasses import dataclass, field
import json
import os
from pathlib import Path
import subprocess
import tempfile

import conformance as h
import oracle

INPUT_TIME = 1600000000000000000
COPY = b'\n\t@cp $< $@\n'
SOURCE = b'source\n'
SIMPLE = b'%.out: %.src' + COPY
CHAIN = b'%.out: %.mid' + COPY + b'%.mid: %.src' + COPY
CLEANUP = b'Removing intermediate files...\n'


@dataclass(frozen=True)
class Model:
    id: str
    makefile: bytes | None
    args: tuple[bytes, ...] = (b'--debug=i', b'a.out')
    files: dict[bytes, bytes] = field(default_factory=dict)
    created: dict[bytes, bytes] = field(default_factory=dict)
    markers: tuple[bytes, ...] = ()
    absent: tuple[bytes, ...] = ()
    status: int = 0
    archive_seed: bool = False


def models():
    yield Model('simple-implicit', SIMPLE, files={b'a.src': SOURCE}, created={b'a.out': SOURCE},
                markers=(b"Trying implicit prerequisite 'a.src'.", b"Found 'a.src'."))
    yield Model('chain', CHAIN, files={b'a.src': SOURCE}, created={b'a.out': SOURCE},
                markers=(b"Not found 'a.mid'.", b'Trying harder.', b"Looking for a rule with intermediate file 'a.mid'.",
                         b"Avoiding implicit rule recursion for rule '%.out: %.mid'.", CLEANUP, b'rm a.mid\n'))
    yield Model('chain-secondary', b'.SECONDEXPANSION:\nS = mid\n%.out: %.$$(S)' + COPY + b'%.mid: %.src' + COPY,
                files={b'a.src': SOURCE}, created={b'a.out': SOURCE},
                markers=(b"Trying pattern rule '%.out: %.$(S)' with stem 'a'.", CLEANUP))
    yield Model('missing-root', SIMPLE, markers=(b"Not found 'a.src'.", b'Trying harder.',
                b"No implicit rule found for 'a.src'.", b"Rejecting rule '%.out: %.src' due to impossible prerequisite 'a.src'."), status=2)
    yield Model('pattern-order', b'%.out: %.missing' + COPY + b'a.%: a.src' + COPY,
                files={b'a.src': SOURCE}, created={b'a.out': SOURCE},
                markers=(b"Not found 'a.missing'.", b"Trying rule prerequisite 'a.src'.", b"Found 'a.src'."))
    yield Model('fallback-after-subchain', b'%.out: %.mid' + COPY + b'%.mid: %.missing' + COPY + b'a.%: a.src' + COPY,
                files={b'a.src': SOURCE}, created={b'a.out': SOURCE}, markers=(b"Not found 'a.mid'.", b"Trying rule prerequisite 'a.src'."))
    yield Model('implicit-with-vpath', b'vpath %.src src\n' + SIMPLE, files={b'src/a.src': SOURCE}, created={b'a.out': SOURCE},
                markers=(b"Found prerequisite 'a.src' as VPATH 'src/a.src'.",))
    yield Model('implicit-directory', SIMPLE, args=(b'--debug=i', b'obj/a.out'),
                files={b'obj/a.src': SOURCE}, created={b'obj/a.out': SOURCE},
                markers=(b"Trying pattern rule '%.out: %.src' with stem 'a'.", b"Found 'obj/a.src'."))
    yield Model('explicit-and-implicit', b'a.out: explicit\n' + SIMPLE,
                files={b'a.src': SOURCE, b'explicit': b'dependency\n'}, created={b'a.out': SOURCE},
                markers=(b"Found 'a.src'.", b"Found implicit rule '%.out: %.src' for 'a.out'."))
    yield Model('cycle-implicit', b'%.out: %.mid' + COPY + b'%.mid: %.out' + COPY,
                markers=(b"does not qualify as ought to exist.", b"Searching for a compatibility rule for 'a.mid'.",
                         b"Using compatibility rule '%.mid: %.out' due to 'a.out'."), status=2)
    yield Model('found-gnumakefile', b'all: ; @false\n', args=(b'--debug=v',),
                files={b'GNUmakefile': b'all: ; @true\n'}, markers=(b"Reading makefile 'GNUmakefile'...",),
                absent=(b"Reading makefile 'Makefile'",))
    yield Model('found-lower-makefile', b'all: ; @true\n', args=(b'--debug=v',),
                markers=(b"Reading makefile 'Makefile'...",))
    yield Model('no-makefile', None, args=(b'--debug=v',), markers=(b'Updating makefiles....',),
                absent=(b'Updating goal targets....',), status=2)
    yield Model('explicit-missing-read', None, args=(b'--debug=m', b'-f', b'first.mk', b'-f', b'second.mk'),
                files={b'second.mk': b'all: ; @true\n'}, markers=(b'Updating makefiles....',), status=2)
    yield Model('generated-include', b"include generated.mk\nall: ; @printf 'restart=%s\\n' '$(MAKE_RESTARTS)'\ngenerated.mk: ; @printf '\\n' > $@\n",
                args=(b'--debug=m',), created={b'generated.mk': b'\n'}, markers=(b'Re-executing[1]:', b'restart=1\n'))
    yield Model('vpath-subchain', b'vpath %.src inputs\n' + CHAIN, files={b'inputs/a.src': SOURCE}, created={b'a.out': SOURCE},
                markers=(b"Found prerequisite 'a.src' as VPATH 'inputs/a.src'.", CLEANUP))
    yield Model('explicit-source-ought', b'a.src:\n' + SIMPLE, files={b'a.src': SOURCE}, created={b'a.out': SOURCE},
                markers=(b"'a.src' ought to exist.",), absent=(b"Found 'a.src'.",))
    yield Model('terminal-missing', b'%.out:: %.src' + COPY, markers=(b"Not found 'a.src'.", b'Trying harder.'),
                absent=(b"Looking for a rule with intermediate file 'a.src'.",), status=2)
    yield Model('order-only-local', b'%.out: | %.src\n\t@cp $| $@\n', files={b'a.src': SOURCE}, created={b'a.out': SOURCE},
                markers=(b"Trying pattern rule '%.out: | %.src'", b"Found 'a.src'."))
    yield Model('cached-impossible', b'%.out: %.src missing-one' + COPY + b'%.out: %.src missing-two' + COPY,
                markers=(b"Rejecting rule '%.out: %.src missing-two' due to impossible implicit prerequisite 'a.src'.",), status=2)
    yield Model('raw-byte-stem', SIMPLE, args=(b'--debug=i', b'\xff.out'), files={b'\xff.src': SOURCE},
                created={b'\xff.out': SOURCE}, markers=(b"with stem '\xff'.", b"Found '\xff.src'."))
    yield Model('kept-intermediate', b'.SECONDARY: a.mid\n' + CHAIN, files={b'a.src': SOURCE},
                created={b'a.out': SOURCE, b'a.mid': SOURCE}, absent=(CLEANUP,))
    yield Model('quiet-intermediate', CHAIN, args=(b'-s', b'--debug=i', b'a.out'), files={b'a.src': SOURCE},
                created={b'a.out': SOURCE}, markers=(CLEANUP,), absent=(b'rm a.mid\n',))
    yield Model('quiet-multiple-intermediates',
                b'%.out: %.last' + COPY + b'%.last: %.mid' + COPY + b'%.mid: %.src' + COPY,
                args=(b'-s', b'--debug=i', b'a.out'), files={b'a.src': SOURCE},
                created={b'a.out': SOURCE}, markers=(CLEANUP,), absent=(b'rm a.mid', b'rm a.last'))
    yield Model('slash-pattern', b'obj/%.out: obj/%.src' + COPY, args=(b'--debug=i', b'obj/a.out'),
                files={b'obj/a.src': SOURCE}, created={b'obj/a.out': SOURCE},
                markers=(b"Trying pattern rule 'obj/%.out: obj/%.src' with stem 'a'.",))

    yield Model('archive-member-pattern-rejected', b'(%): %.obj' + COPY,
                args=(b'--debug=i', b'lib.a(a.o)'), files={b'a.obj': SOURCE}, status=2,
                markers=(b"Looking for archive-member implicit rule for 'lib.a(a.o)'.",
                         b"Trying pattern rule '(%): %.obj' with stem 'a.o'.",
                         b"No implicit rule found for '(a.o)'.",
                         b"No archive-member implicit rule found for 'lib.a(a.o)'."))
    yield Model('archive-member-pattern-found', b'(%): %.obj\n\t@cp $< marker\n',
                args=(b'--debug=i', b'lib.a(a.o)'), files={b'a.o.obj': SOURCE}, created={b'marker': SOURCE},
                markers=(b"Found implicit rule '(%): %.obj' for '(a.o)'.",))
    yield Model('archive-member-no-rule', b'all: ; @true\n', args=(b'--debug=i', b'lib.a(a.o)'), status=2,
                markers=(b"No implicit rule found for 'lib.a(a.o)'.",
                         b"No implicit rule found for '(a.o)'."))
    yield Model('archive-member-zero-date', b'all: ; @true\n', args=(b'--debug=i', b'lib.a(a.o)'),
                files={b'a.o': SOURCE}, archive_seed=True, status=2,
                markers=(b"No archive-member implicit rule found for 'lib.a(a.o)'.",))
    for goal in (b'a.one', b'a.two'):
        yield Model('grouped-pattern-' + goal[2:].decode(),
                    b"%.one %.two: %.src\n\t@cp $< '$*.one'; cp $< '$*.two'\n",
                    args=(b'--debug=i', goal), files={b'a.src': SOURCE}, created={b'a.one': SOURCE, b'a.two': SOURCE},
                    markers=(b"Trying pattern rule '%.one %.two: %.src' with stem 'a'.",
                             b"Found implicit rule '%.one %.two: %.src' for '" + goal + b"'."))
    yield Model('literal-directory-prereq', b'%.out: fixed' + COPY, args=(b'--debug=i', b'sub/a.out'),
                files={b'fixed': SOURCE, b'sub/.keep': b''}, created={b'sub/a.out': SOURCE},
                markers=(b"Trying rule prerequisite 'fixed'.",), absent=(b"Trying implicit prerequisite 'fixed'.",))
    yield Model('explicit-dot-prereq', b'%.out: ./a.src' + COPY, files={b'a.src': SOURCE},
                created={b'a.out': SOURCE})
    yield Model('secondary-literal', b'.SECONDEXPANSION:\nS = a.src\n%.out: $$(S)' + COPY,
                files={b'a.src': SOURCE}, created={b'a.out': SOURCE})


def observe(model, executable, invoke, reset):
    reset.mkdir()
    work, home, temporary = (reset / name for name in ('work', 'home', 'tmp'))
    for directory in (work, home, temporary):
        directory.mkdir()
    inputs = dict(model.files)
    if model.makefile is not None:
        inputs[b'Makefile'] = model.makefile
    for name, data in inputs.items():
        path = work / os.fsdecode(name)
        path.parent.mkdir(exist_ok=True, parents=True)
        path.write_bytes(data)
        os.utime(path, ns=(INPUT_TIME, INPUT_TIME))
    if model.archive_seed:
        subprocess.run(['/usr/bin/ar', 'rcsD', 'lib.a', 'a.o'], cwd=work, check=True, capture_output=True)
        archive = work / 'lib.a'
        os.utime(archive, ns=(INPUT_TIME, INPUT_TIME))
        inputs[b'lib.a'] = archive.read_bytes()
    invoke.unlink(missing_ok=True)
    invoke.symlink_to(executable)
    environment = oracle.clean_environment(home) | {'TMPDIR': str(temporary)}
    out, err, status, limit, children = h.capture([str(invoke), '-rR', '--no-print-directory', *model.args],
                                                  work, environment, 12)
    files = h.snapshot({'work': work, 'home': home, 'tmp': temporary})
    actual_files = {os.fsencode(path.relative_to(work)): path.read_bytes()
                    for path in work.rglob('*') if path.is_file()}
    tests = [('status or containment', (status, limit, children) == (model.status, None, False)),
             ('expected file effects', actual_files == inputs | model.created),
             ('selected positive marker', all(marker in out for marker in model.markers)),
             ('selected absent marker', all(marker not in out for marker in model.absent))]
    return {'stdout_hex': out.hex(), 'stderr_hex': err.hex(), 'status': status, 'limit': limit,
            'leftover_processes': children, 'filesystem': files,
            'errors': [name for name, success in tests if not success]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case')
    args = parser.parse_args()
    selected = [model for model in models() if args.case is None or args.case == model.id]
    if not selected:
        parser.error(f'unknown debug traversal case: {args.case}')
    profile = oracle.verify(args.oracle)
    rows = []
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-debug-traversal-') as directory:
            root = Path(directory)
            for model in selected:
                observations = []
                reset = root / 'reset'
                for executable in (args.oracle, args.oracle, args.candidate):
                    try:
                        observations.append(observe(model, executable.absolute(), root / 'make', reset))
                    finally:
                        h.remove_tree(reset)
                passed = not observations[0]['errors'] and observations[0] == observations[1] == observations[2]
                print('PASS' if passed else 'FAIL', model.id, [row['errors'] for row in observations], flush=True)
                rows.append({'id': model.id, 'pass': passed, 'observations': observations})
    finally:
        os.umask(previous_umask)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    passed = sum(row['pass'] for row in rows)
    args.report.write_text(json.dumps({'cases': len(rows), 'passed': passed, 'results': rows,
                                      'oracle_profile': profile['profile_id'],
                                      'candidate_sha256': h.sha256_file(args.candidate)}, indent=2) + '\n')
    print(f'Debug traversal gate: {passed}/{len(rows)}', flush=True)
    return int(passed != len(rows))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (h.HarnessError, oracle.OracleError, OSError) as error:
        raise SystemExit(f'debug traversal gate: {error}') from error
