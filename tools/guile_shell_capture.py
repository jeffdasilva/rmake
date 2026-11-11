#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Compare detached Guile callbacks during a verified shell capture."""
import argparse
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import guile_frontend as g

DATA = g.ROOT / 'tests/guile-shell-capture'


def capture_wait(pid: int, *, reference: bool, nested: bool) -> dict | None:
    proc = Path('/proc') / str(pid)
    expected = b'/bin/sh\0capture.sh\0' + (b'inner\0' if nested else b'')
    try:
        threads = list((proc / 'task').iterdir()) if reference and nested else [proc / 'task' / str(pid)]
    except OSError:
        return None
    for thread in threads:
        try:
            raw = (thread / 'syscall').read_text().split()
            state = (thread / 'stat').read_text().rpartition(') ')[2].split()[0]
            if state != 'S' or len(raw) != 9:
                continue
            for ident in (thread / 'children').read_text().split():
                child = Path('/proc') / ident
                if (child / 'cmdline').read_bytes() != expected:
                    continue
                stat = (child / 'stat').read_text().rpartition(') ')[2].split()
                if int(stat[1]) != pid or int(stat[19]) <= 0:
                    continue
                pipe = os.readlink(child / 'fd' / '1')
                if not pipe.startswith('pipe:['):
                    continue
                if reference:
                    reading = raw[0] == '0' and os.readlink(proc / 'fd' / str(int(raw[1], 0))) == pipe
                else:
                    reading = (raw[0] == '7' and (int(raw[3], 0) & 0xffffffff) != 0
                               and pipe in {os.readlink(fd) for fd in (proc / 'fd').iterdir()})
                if reading:
                    return {'parent': pid, 'thread': int(thread.name), 'child': int(ident),
                            'child_start': int(stat[19]), 'pipe': pipe, 'syscall': raw}
        except (OSError, ValueError, IndexError):
            # A child can change state while procfs is being observed.
            continue
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    if os.uname().machine != 'x86_64':
        parser.error('the capture witness requires Linux x86-64 syscall identities')
    if args.report.exists():
        parser.error('refusing to replace an existing report')
    profile = g.read_profile(args.profile)
    reference = Path(profile['binary']['path'])
    candidate = args.candidate.resolve(strict=True) if args.candidate else None
    tools = [reference, reference, *([candidate] if candidate else [])]
    environment = {key: profile['environment'][key] for key in g.ENVIRONMENT}
    environment['GNUMAKEFLAGS'] = ''
    model = DATA / 'models.json'
    script = DATA / 'capture.sh'
    models = json.loads(model.read_text())['cases']
    if len(models) != 15 or len({row['id'] for row in models}) != len(models):
        raise h.HarnessError('invalid Guile capture model inventory')
    results = []
    previous = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-guile-shell-capture-') as temp:
            root = Path(temp)
            runtime = root / 'runtime'
            runtime.mkdir()
            work = runtime / 'work'
            roots = {name: runtime / name for name in ('work', 'home', 'tmp')}
            for row in models:
                ident = row['id']
                fixture = root / g.safe_relative(ident)
                fixture.mkdir()
                (fixture / 'Makefile').write_text(row['makefile'])
                (fixture / 'capture.sh').write_bytes(script.read_bytes())
                argv = tuple(row.get('argv', ('-rR', '--no-print-directory', '-j1')))
                case = h.Case(fixture, ident, 'guile.shell-capture', 'required',
                              argv, 8, environment, {})
                h.reset_workspace(case, runtime, reference)
                filesystem = h.snapshot(roots)
                effects = {'capture-ready': b'waiting\n', 'release': b'go\n',
                           'worker-result': row['worker_result'].encode()}
                gates = [('capture-ready', 'release', False)]
                if row.get('nested'):
                    effects.update({'inner-ready': b'waiting\n', 'inner-release': b'go\n',
                                    'inner-result': b'global\n'})
                    gates.append(('inner-ready', 'inner-release', True))
                for name, content in effects.items():
                    filesystem['work/' + name] = {'mode': 0o644, 'type': 'file',
                                                   **g.file_fact(content)}
                expected = h.Observation(row['stdout'].encode(), b'', 0, None, filesystem, False)
                runs = []
                for index, binary in enumerate(tools):
                    releases = []

                    def observe(pid, _stdout):
                        if len(releases) == len(gates):
                            return
                        ready, release, nested = gates[len(releases)]
                        marker = work / ready
                        if not marker.is_file() or marker.read_bytes() != b'waiting\n':
                            return
                        witness = capture_wait(pid, reference=index < 2, nested=nested)
                        if witness is not None:
                            staging = work / (release + '.tmp')
                            staging.write_bytes(b'go\n')
                            staging.rename(work / release)
                            releases.append(witness)

                    observed = h.execute(case, runtime, binary, observe=observe)
                    errors = h.differences(expected, observed)
                    if len(releases) != len(gates):
                        errors.append('missing witnessed capture release')
                    runs.append({'errors': errors, 'releases': releases, 'stdout': observed.stdout.hex(),
                                 'stderr': observed.stderr.hex(), 'status': observed.returncode,
                                 'limit': observed.limit, 'leftover': observed.leftover_processes,
                                 'filesystem': observed.filesystem})
                passed = all(not run['errors'] for run in runs)
                results.append({'case': ident, 'passed': passed, 'runs': runs})
                print('PASS' if passed else 'FAIL', ident, flush=True)
    finally:
        os.umask(previous)
    passed = sum(row['passed'] for row in results)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps({
        'oracle': profile['binary'], 'candidate': h.executable_identity(candidate) if candidate else None,
        'model_sha256': h.sha256_file(model), 'script_sha256': h.sha256_file(script),
        'results': results, 'passed': passed,
    }, indent=2) + '\n')
    print(f'Guile shell capture: {passed}/{len(results)} passed', flush=True)
    return int(passed != len(results))


if __name__ == '__main__':
    raise SystemExit(main())
