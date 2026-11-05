#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Exercise real top-level bootstrap, clean and benchmark preconditions."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import tempfile

import conformance as h
import oracle
import rust

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/developer-flows'
BUILD_LINE = b'python3 tools/rust.py cargo build --workspace --release --locked\n'
CLEAN_LINE = b'python3 tools/rust.py cargo clean\n'
FINISHED = re.compile(rb'(?m)^    Finished `release` profile \[optimized\] target\(s\) in ([0-9]+(?:\.[0-9]+)?s)\n')
CLEANED = re.compile(rb'     Removed [1-9][0-9]* files?, [0-9.]+[A-Za-z]+ total\n')
CLEAN_CASES = {
    'clean-ordinary': ((), False, False),
    'clean-parallel-pipe': (('-j2', '--jobserver-style=pipe'), False, False),
    'clean-dry-run': (('-n',), True, False),
    'clean-question': (('-q',), True, False),
    'clean-touch': (('-t',), True, False),
    'clean-default-directory': ((), False, True),
}
CASE_IDS = ('warm-bootstrap-oracle', *CLEAN_CASES,
            'bench-invalid-repeat', 'bench-invalid-suite', 'bench-compare-missing-executable')


def require(value, description):
    if not value:
        raise h.HarnessError(description)


def execute(executable, work, env, args):
    return h.capture([str(executable), '--no-print-directory', *args], work, env, 40)


def release_stderr(stderr):
    hits = list(FINISHED.finditer(stderr))
    require(len(hits) == 1, 'expected a single Cargo completion field')
    start, stop = hits[0].span(1)
    return stderr[:start] + b'<cargo-elapsed>' + stderr[stop:]


def digest_tree(root):
    if not root.exists():
        return None
    return h.snapshot({'tree': root})


class Subject:
    def __init__(self, root, binary, profile):
        self.root = root
        self.work = root / 'work'
        self.home = root / 'home'
        self.tmp = root / 'tmp'
        for directory in (self.work, self.home, self.tmp):
            directory.mkdir()
        self.environment = oracle.clean_environment(self.home) | {
            'TMPDIR': str(self.tmp), 'CARGO_NET_OFFLINE': 'true',
            'CARGO_TERM_COLOR': 'never', 'CARGO_TERM_PROGRESS_WHEN': 'never',
        }
        for name in ('Cargo.toml', 'Cargo.lock', 'src/main.rs'):
            target = self.work / name
            target.parent.mkdir(exist_ok=True)
            shutil.copy2(FIXTURE / name, target)
        for name in ('Makefile', 'tools/rust.py', 'tools/oracle.py', 'tools/benchmark.py',
                     'tools/conformance.py', 'oracle/manifest.json'):
            target = self.work / name
            target.parent.mkdir(exist_ok=True)
            shutil.copy2(ROOT / name, target)
        tools = self.work / '.tools'
        oracle_root = tools / 'oracle'
        (oracle_root / 'bin').mkdir(parents=True)
        shutil.copy2(binary, oracle_root / 'bin/make')
        shutil.copy2(oracle.PROFILE, oracle_root / 'profile.json')
        self.verified = (
            f"Verified GNU Make 4.4.1: {oracle_root / 'bin/make'}\n"
            f"Profile {profile['profile_id']}; binary SHA256 {profile['binary']['sha256']}\n"
        ).encode()
        # The pinned installed compiler is an input. Cargo cache and target
        # paths remain private to this subject; neither clean nor bootstrap
        # is allowed to write to the real checkout.
        toolchain = (ROOT / '.tools/rustup/toolchains' /
                     f'{rust.TOOLCHAIN}-x86_64-unknown-linux-gnu' / 'bin')
        cargo_bin = tools / 'cargo/bin'
        cargo_bin.mkdir(parents=True)
        for name in ('cargo', 'rustc'):
            executable = toolchain / name
            require(os.access(executable, os.X_OK), 'run make bootstrap-rust first')
            (cargo_bin / name).symlink_to(executable)
        self.target = self.work / 'target'
        self.explicit_target = root / 'selected-cargo-target'
        self.seed = root / 'clean-seed'
        self.release = root / 'benchmark-target'
        self.neighbor = root / 'unrelated-target'
        self.neighbor.mkdir()
        (self.neighbor / 'keep').write_bytes(b'unrelated target bytes\n')
        (self.home / '.profile').write_bytes(b'# preserve private home\n')
        self.profile = oracle_root
        invocation = root / 'invocation'
        invocation.mkdir()
        self.engine = invocation / 'make'

    def run(self, binary, *args, target=None):
        env = self.environment.copy()
        if target is not None:
            require(target.is_relative_to(self.root), 'refusing a target outside the subject')
            env['CARGO_TARGET_DIR'] = str(target)
        self.engine.unlink(missing_ok=True)
        self.engine.symlink_to(binary)
        return execute(self.engine, self.work, env, list(args))

    def protected(self):
        files = ('Makefile', 'Cargo.toml', 'Cargo.lock', 'tools/rust.py',
                 'tools/oracle.py', 'tools/benchmark.py', 'tools/conformance.py',
                 'oracle/manifest.json')
        return {
            'files': {name: h.sha256_file(self.work / name) for name in files},
            'profile': digest_tree(self.profile), 'source': digest_tree(self.work / 'src'),
            'neighbor': digest_tree(self.neighbor), 'home': digest_tree(self.home),
            'temporary': digest_tree(self.tmp),
        }

    def build_inputs(self, oracle_binary, selected):
        if set(selected) & CLEAN_CASES.keys():
            out, err, status, limit, children = self.run(
                oracle_binary, '-j2', '--jobserver-style=pipe', 'build', target=self.seed)
            require((status, limit, children) == (0, None, False), f'Cargo clean seed: {err!r}')
            require(b'cargo build --workspace --locked' in out, 'Cargo build prerequisite did not run')
            require((self.seed / 'debug/rmake').read_bytes()[:4] == b'\x7fELF',
                    'Cargo did not build a real executable')
        if 'bench-compare-missing-executable' in selected:
            out, err, status, limit, children = self.run(oracle_binary, 'release', target=self.release)
            require((status, limit, children) == (0, None, False), f'Cargo benchmark seed: {err!r}')
            require(BUILD_LINE in out, 'Cargo release prerequisite did not run')
            require((self.release / 'release/rmake').read_bytes()[:4] == b'\x7fELF',
                    'Cargo did not build a real release executable')

    def observe(self, case, binary):
        before = self.protected()
        if case == 'warm-bootstrap-oracle':
            result = self.run(binary, 'bootstrap-oracle')
            want = b'python3 tools/oracle.py bootstrap\n' + self.verified
            require(result == (want, b'', 0, None, False), f'warm bootstrap model: {result!r}')
            require((self.work / '.tools/oracle.lock').is_file(), 'oracle did not take its lock')
            require(not (self.work / '.tools/downloads').exists(), 'warm oracle attempted a download')
        elif case in CLEAN_CASES:
            flags, preserved, default = CLEAN_CASES[case]
            target = self.target if default else self.explicit_target
            other = self.explicit_target if default else self.target
            require(not target.exists() and not other.exists(), 'clean paths must start absent')
            other.mkdir()
            (other / 'keep').write_bytes(b'preserve the unselected Cargo target\n')
            other_before = digest_tree(other)
            shutil.copytree(self.seed, target, symlinks=True)
            target_before = digest_tree(target)
            try:
                result = self.run(binary, *flags, 'clean', target=None if default else target)
                out, err, status, limit, children = result
                require((out, status, limit, children) == (CLEAN_LINE, 0, None, False),
                        f'clean model: {result!r}')
                require(err == b'' if preserved else CLEANED.fullmatch(err), f'clean stderr: {err!r}')
                require(digest_tree(target) == (target_before if preserved else None),
                        'clean target contents did not match selected mode')
                require(digest_tree(other) == other_before, 'clean touched the unselected Cargo target')
            finally:
                if other.exists():
                    shutil.rmtree(other)
                if target.exists():
                    shutil.rmtree(target)
        else:
            missing = self.root / 'absent-candidate'
            compare = case == 'bench-compare-missing-executable'
            if compare:
                args = (f'BENCH_CANDIDATE={missing}', 'BENCH_REPEAT=3', 'bench-compare')
                diagnostic = b'executable not found or not executable: ' + os.fsencode(missing)
            elif case == 'bench-invalid-repeat':
                args = ('BENCH_REPEAT=2', 'bench')
                diagnostic = b'--repeat must be between 3 and 1000'
            else:
                args = ('BENCH_SUITE=unselected-suite', 'bench')
                diagnostic = b"invalid choice: 'unselected-suite'"
            release_path = self.release / 'release/rmake'
            release_sha = h.sha256_file(release_path) if compare else None
            out, err, status, limit, children = self.run(binary, *args, target=self.release)
            require((status, limit, children) == (2, None, False) and err.count(diagnostic) == 1,
                    f'benchmark precondition model: {out!r} {err!r}')
            require(b'python3 tools/oracle.py verify ' in out and self.verified in out and
                    b'python3 tools/benchmark.py ' in out and (BUILD_LINE in out) == compare,
                    'benchmark flow skipped or added a real prerequisite')
            require(not (self.work / 'artifacts/bench').exists() and not missing.exists(),
                    'benchmark precondition produced an unexpected result file')
            if compare:
                require(h.sha256_file(release_path) == release_sha, 'warm release executable changed')
                err = release_stderr(err)
            result = (out, err, status, limit, children)
        require(self.protected() == before, 'developer flow changed protected inputs')
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--case', choices=CASE_IDS)
    args = parser.parse_args()
    binary = args.oracle.absolute()
    candidate = args.candidate.absolute()
    profile = oracle.verify(binary)
    require(os.access(candidate, os.X_OK), 'candidate executable is unavailable')
    selected = (args.case,) if args.case else CASE_IDS
    results = []
    previous_umask = os.umask(0o022)
    try:
        with tempfile.TemporaryDirectory(prefix='rmake-developer-flows-') as directory:
            subject = Subject(Path(directory), binary, profile)
            subject.build_inputs(binary, selected)
            for case in selected:
                runs, errors = [], []
                for name, executable in (('gnu-first', binary), ('gnu-second', binary), ('candidate', candidate)):
                    try:
                        output = subject.observe(case, executable)
                        runs.append((name, output))
                    except h.HarnessError as error:
                        errors.append(f'{name}: {error}')
                qualified = len(runs) == 3 and all(output == runs[0][1] for _, output in runs)
                results.append({'id': case, 'pass': qualified, 'errors': errors, 'runs': [
                    {'engine': name, 'stdout_hex': out.hex(), 'stderr_hex': err.hex(),
                     'status': status, 'limit': limit, 'leftover_processes': children}
                    for name, (out, err, status, limit, children) in runs]})
                print('PASS' if qualified else 'FAIL', case, errors, flush=True)
    finally:
        os.umask(previous_umask)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    passed = sum(row['pass'] for row in results)
    args.report.write_text(json.dumps({'passed': passed, 'cases': len(results),
                                      'oracle_profile': profile['profile_id'],
                                      'candidate_sha256': h.sha256_file(candidate),
                                      'makefile_sha256': h.sha256_file(ROOT / 'Makefile'),
                                      'results': results}, indent=2) + '\n')
    print(f'Developer flow gate: {passed}/{len(results)}', flush=True)
    return int(passed != len(results))


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (h.HarnessError, oracle.OracleError, OSError) as error:
        raise SystemExit(f'developer flow gate: {error}') from error
