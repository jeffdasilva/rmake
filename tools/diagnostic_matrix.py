#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva.
"""Original byte-exact info/warning function matrix against GNU Make 4.4.1.

Every cell has authored output, status, and complete filesystem expectations.
The programs were created independently without inspecting GNU implementation
source or tests. Only trusted, bounded fixtures run here; process containment
is not a filesystem or network security sandbox.
"""
from __future__ import annotations

import argparse
import base64
import dataclasses
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
from typing import Any

import conformance
import oracle

MATRIX_VERSION = 1
DEFAULT_MAX_FAILURES = 20
PROTECTED = b"Original diagnostic-effect campaign anchor.\n"
LIMITATIONS = [
    "trusted fixed inputs; process containment is not a security sandbox",
    "single invocation, serial recipes, timestamps not compared",
    "multiple-export traversal and effect-bearing MAKEFLAGS/GNUMAKEFLAGS excluded; curated pending cases retain these boundaries",
    "fixed info/warning examples, not exhaustive expansion compatibility",
]


@dataclasses.dataclass(frozen=True)
class Cell:
    case_id: str
    family: str
    makefile: bytes
    args: tuple[str, ...]
    stdout: bytes
    stderr: bytes
    status: int
    output: str
    created: tuple[tuple[str, bytes], ...]

    def metadata(self) -> dict[str, Any]:
        return {
            "case_id": self.case_id, "family": self.family,
            "matrix_version": MATRIX_VERSION, "argv_after_alias": list(self.args),
            "makefile_sha256": hashlib.sha256(self.makefile).hexdigest(),
            "protected_input_sha256": hashlib.sha256(PROTECTED).hexdigest(),
            "expected_stdout_hex": self.stdout.hex(),
            "expected_stderr_hex": self.stderr.hex(),
            "expected_status": self.status, "output": self.output,
            "expected_created_hex": {name: data.hex() for name, data in self.created},
        }


def message(kind: bytes, text: bytes, line: int | None = None) -> bytes:
    if kind == b"info":
        return text + b"\n"
    prefix = b"Makefile:" + str(line).encode() + b": " if line else b"make: "
    return prefix + text + b"\n"


def expr(kind: bytes, text: bytes) -> bytes:
    return b"$(" + kind + b" " + text + b")"


def spec(family: str, source: bytes, stdout: bytes = b"", stderr: bytes = b"",
         status: int = 0, args: tuple[bytes, ...] = (),
         created: dict[str, bytes] | None = None, merged: bool = False) -> Cell:
    return Cell(
        case_id="", family=family, makefile=source,
        args=tuple(value.decode("ascii") for value in (b"-rR", b"--no-print-directory", *args)),
        stdout=stdout, stderr=stderr, status=status,
        output="merged" if merged else "separate",
        created=tuple((created or {}).items()),
    )


def effect_spec(family: str, source: bytes, kind: bytes, text: bytes,
                line: int | None) -> Cell:
    effect = message(kind, text, line)
    return spec(family, source, stdout=effect if kind == b"info" else b"",
                stderr=effect if kind == b"warning" else b"")


def inventory() -> tuple[Cell, ...]:
    cases = []
    tail = b'all:\n\t@/bin/true\n'
    payloads = [
        b'',
        b'plain',
        b' leading ',
        b'\tindented\t',
        b'one,two,three',
        b'$literal $(unexpanded)',
        b'quotes \'" slash \\',
        b'\xff\xfe',
        b'line1\nline2',
        b'\n',
        b'\x0bform\x0c',
        b'\rCR\r',
        b'. Stop.',
        b'x' * 4096,
        b'% @ # : ; | >',
        b'unicode \xce\xbb',
    ]
    for kind in (b'info', b'warning'):
        for payload in payloads:
            # Padding keeps a trailing CR inside the value rather than a CRLF line ending.
            source = (b'define MESSAGE\nX' + payload + b'X\nendef\n'
                      + expr(kind, b'$(subst X,,$(value MESSAGE))') + b'\n' + tail)
            cases.append(effect_spec('raw-message-bytes', source, kind, payload, 4 + payload.count(b'\n')))
        for separator in (b' ', b'\t', b'\x0b', b'\x0c', b'\r'):
            source = b'$(' + kind + separator + b'  text  )\n' + tail
            cases.append(effect_spec('function-delimiter', source, kind, b'text  ', 1))
        for padding in (0, 3):
            pad = b'# padding\n' * padding
            contexts = [
                (expr(kind, b'context') + b'\n' + tail, 1),
                (b'X := ' + expr(kind, b'context') + b'\n' + tail, 1),
                (b'X = ' + expr(kind, b'context') + b'\n$(X)\n' + tail, 2),
                (b'X = ' + expr(kind, b'context') + b'\nall:\n\t@$(X)/bin/true\n', 3),
                (b'define X\n' + expr(kind, b'context') + b'\nendef\nall:\n\t@$(X)/bin/true\n', 5),
                (b'X = ' + expr(kind, b'context') + b'\nY = $(call X)\nall:\n\t@$(call Y)/bin/true\n', 4),
                (b'X = ' + expr(kind, b'context') + b'\nexport Y = $(X)\n' + tail, 2),
                (b'.DEFAULT_GOAL = ' + expr(kind, b'context') + b'all\n' + tail, None),
                (b'X = unused\nX = ' + expr(kind, b'context') + b'\nall:\n\t@$(X)/bin/true\n', 4),
                (b'.ONESHELL:\nX = ' + expr(kind, b'context') + b'\nall:\n\t@/bin/true\n\t@$(X)/bin/true\n', 4),
            ]
            for body, line in contexts:
                cases.append(effect_spec(
                    'source-contexts', pad + body, kind, b'context',
                    None if line is None else padding + line,
                ))
        callforms = [
            (b'$(call ' + kind + b')', []),
            (b'$(call ' + kind + b',)', [b'']),
            (b'$(call ' + kind + b',one,two)', [b'one']),
            (b'$(call ' + kind + b',one,' + expr(kind, b'extra') + b')', [b'extra', b'one']),
            (b'$(' + kind + b')', []),
            (b'${' + kind + b' brace}', [b'brace']),
            (b'$(call ' + kind + b',' + expr(kind, b'inner') + b')', [b'inner', b'']),
        ]
        for form, outputs in callforms:
            source = form + b'\n' + tail
            expected = b''.join((message(kind, t, 1) for t in outputs))
            cases.append(spec(
                'call-arity', source,
                expected if kind == b'info' else b'', expected if kind == b'warning' else b''))
        effect = expr(kind, b'chosen')
        lazy = [
            (b'$(if yes,' + effect + b',$(error unused))', True),
            (b'$(if ,$(error unused),' + effect + b')', True),
            (b'$(if ,' + effect + b')', False),
            (b'$(or yes,' + effect + b')', False),
            (b'$(or ,' + effect + b')', True),
            (b'$(and ,' + effect + b')', False),
            (b'$(and yes,' + effect + b')', True),
            (b'$(foreach x,,' + effect + b')', False),
            (b'$(foreach x,a b,' + effect + b')', 2),
            (b'$(intcmp 1,2,' + effect + b',$(error unused),$(error unused))', True),
        ]
        for form, count in lazy:
            source = b'X := ' + form + b'\n' + tail
            expected = message(kind, b'chosen', 1) * int(count)
            cases.append(spec(
                'lazy-control-flow', source,
                expected if kind == b'info' else b'', expected if kind == b'warning' else b''))
        nested = [
            (expr(kind, b'outer-' + expr(kind, b'inner') + b'-end'), [b'inner', b'outer--end']),
            (expr(kind, b'a,' + expr(kind, b'b') + b',c'), [b'b', b'a,,c']),
            (b'$(foreach x,a b,' + expr(kind, b'$(x)') + b')', [b'a', b'b']),
            (b'$(call ' + kind + b',first,' + expr(kind, b'second') + b','
             + expr(kind, b'third') + b')', [b'second', b'third', b'first']),
        ]
        for form, outputs in nested:
            expected = b''.join((message(kind, t, 1) for t in outputs))
            cases.append(spec(
                'nested-effects', form + b'\n' + tail,
                expected if kind == b'info' else b'', expected if kind == b'warning' else b''))
        for mode in ('immediate', 'recursive', 'export'):
            assignment = b'X:=' + expr(kind, b'cli') if mode == 'immediate' else b'X=' + expr(kind, b'cli')
            source = b'all:\n\t@$(X)/bin/true\n' if mode == 'recursive' else tail
            # A CLI recursive binding expands once in the body and again for child exports.
            if mode == 'recursive':
                out = message(kind, b'cli', 2) + message(kind, b'cli', None)
            else:
                out = message(kind, b'cli', None)
            cases.append(spec(
                'command-line-context', source,
                out if kind == b'info' else b'', out if kind == b'warning' else b'', args=(assignment,)))
        for position in range(5):
            effect = expr(kind, b'before')
            fatal = b'$(error stopped)'
            forms = [
                effect + fatal,
                expr(kind, b'outer' + fatal),
                b'$(call ' + kind + b',first,' + fatal + b')',
                b'$(call ' + kind + b',' + effect + b',' + fatal + b')',
                b'$(if yes,' + effect + fatal + b',' + expr(kind, b'unused') + b')',
            ]
            source = forms[position] + b'\n' + tail
            emits_before_fatal = position in (0, 3, 4)
            before_fatal = message(kind, b'before', 1) if emits_before_fatal else b''
            cases.append(spec(
                'output-before-fatal', source,
                before_fatal if kind == b'info' else b'',
                (before_fatal if kind == b'warning' else b'') + b'Makefile:1: *** stopped.  Stop.\n',
                status=2,
            ))
    modes = [
        ((), b'@'),
        ((b'-n',), b'@'),
        ((b'-q',), b'@'),
        ((b'-t',), b'@'),
        ((b'-n',), b'+@'),
        ((b'-q',), b'+@'),
        ((b'-t',), b'+@'),
    ]
    for oneshell in (False, True):
        for args, prefix in modes:
            source = ((b'.ONESHELL:\n' if oneshell else b'') + b'all:\n\t' + prefix
                      + b'$(info first)/bin/true\n\t@$(warning second)/bin/true\n')
            unforced_touch = b'-t' in args and b'+' not in prefix
            out = b'' if unforced_touch else b'first\n' + message(b'warning', b'second', 3)
            status = int(b'-q' in args and (not (oneshell and b'+' in prefix)))
            created = {}
            if b'-n' in args:
                out += b'/bin/true\n/bin/true\n'
            if b'-t' in args and (b'+' not in prefix or not oneshell):
                out += b'touch all\n' * (2 if b'+' in prefix else 1)
                created = {'all': b''}
            cases.append(spec('recipe-mode-order', source, out, status=status, args=args, created=created, merged=True))
    for kind in (b'info', b'warning'):
        for policy in ('explicit', 'unexport', 'implicit'):
            header = b'X = ' + expr(kind, b'export-effect') + b'\n'
            if policy == 'explicit':
                header += b'export X\n'
            elif policy == 'unexport':
                header += b'unexport X\n'
            for args, prefix in modes[:6]:
                source = header + b'all:\n\t' + prefix + b'/bin/true\n'
                forced = b'+' in prefix
                launch = not args or forced
                out = b''
                err = b''
                created = {}
                status = 0
                if b'-n' in args:
                    out = b'/bin/true\n'
                if b'-q' in args and (not forced):
                    status = 1
                if b'-t' in args:
                    out = b'touch all\n'
                    created = {'all': b''}
                if launch and policy == 'explicit':
                    emission = message(kind, b'export-effect', 1)
                    if kind == b'info':
                        out += emission
                    else:
                        err += emission
                cases.append(spec('export-effects-modes', source, out, err, status, args, created=created))
        for config, tailvalue in ((b'SHELL', b'/bin/sh'), (b'.SHELLFLAGS', b'-c'), (b'IFS', b' ')):
            for args in ((), (b'-n',)):
                source = config + b' = ' + expr(kind, b'config') + tailvalue + b'\nall:\n\t@/bin/true\n'
                out = message(kind, b'config', 3)
                cases.append(spec(
                    'shell-configuration-effects', source,
                    (out if kind == b'info' else b'') + (b'/bin/true\n' if args else b''),
                    out if kind == b'warning' else b'', args=args,
                ))
    # Parser diagnostics must stay between the user effects that surround them.
    parser = b'$(info before)\nall: ; @/bin/true\n$(warning between)\nall: ; @/bin/true\n'
    parser_out = (b"before\nMakefile:3: between\n"
                  b"Makefile:4: warning: overriding recipe for target 'all'\n"
                  b"Makefile:2: warning: ignoring old recipe for target 'all'\n")
    cases.append(spec('parser-ordering', parser + b'$(info after)\n', parser_out + b'after\n', merged=True))
    cases.append(spec(
        'parser-ordering', parser + b'$(error stopped)\n',
        parser_out + b'Makefile:5: *** stopped.  Stop.\n', status=2, merged=True,
    ))
    source = b'$(warning parse)\nall:\n\t@$(info expand1)printf "child1\\n"\n\t@$(warning expand2)printf "child2\\n"\n'
    cases.append(spec('child-output-order', source, b'Makefile:1: parse\nexpand1\nMakefile:4: expand2\nchild1\nchild2\n', merged=True))
    source = b'$(info before)\n$(warning middle)\n$(error stopped)\n' + tail
    cases.append(spec(
        'mixed-fatal-order', source,
        b'before\nMakefile:2: middle\nMakefile:3: *** stopped.  Stop.\n', status=2, merged=True,
    ))
    return tuple(
        dataclasses.replace(cell, case_id=f"message-{index:03d}-{cell.family}")
        for index, cell in enumerate(cases)
    )



def select_cells(case_ids: list[str]) -> tuple[Cell, ...]:
    cells = inventory()
    requested = set(case_ids)
    unknown = requested - {cell.case_id for cell in cells}
    if unknown:
        raise conformance.HarnessError(f"unknown case IDs: {sorted(unknown)}")
    return tuple(cell for cell in cells if not requested or cell.case_id in requested)


def validate_limit(max_failures: int) -> None:
    if type(max_failures) is not int or not 1 <= max_failures <= len(inventory()):
        raise conformance.HarnessError(f"max-failures must be an integer between 1 and {len(inventory())}")


def regular_file(content: bytes) -> dict[str, Any]:
    return {"type": "file", "mode": 0o644, "size": len(content),
            "sha256": hashlib.sha256(content).hexdigest(),
            "content": base64.b64encode(content).decode("ascii")}


def metadata_bytes(cell: Cell) -> bytes:
    return (json.dumps(cell.metadata(), indent=2, sort_keys=True) + "\n").encode()


def materialize(cell: Cell, parent: Path) -> conformance.Case:
    path = parent / cell.case_id
    path.mkdir(parents=True)
    files = {"Makefile": cell.makefile, "protected.txt": PROTECTED,
             "matrix.json": metadata_bytes(cell)}
    for name, data in files.items():
        (path / name).write_bytes(data)
        (path / name).chmod(0o644)
    (path / "case.toml").write_text(
        f'id = "{cell.case_id}"\nrequirement = "matrix.diagnostic-effects"\n'
        'candidate = "required"\nargs = ' + json.dumps(cell.args) + '\n'
        f'timeout_seconds = 5\noutput = "{cell.output}"\n'
        'provenance = "Original independently authored GNU4.4.1 message observations; no upstream source or tests copied."\n'
        f'[expected]\nexit_code = {cell.status}\n'
        f'stdout_hex = "{cell.stdout.hex()}"\nstderr_hex = "{cell.stderr.hex()}"\n')
    return conformance.load_case(path)


def expected_files(cell: Cell) -> dict[str, Any]:
    result = {name: {"type": "directory", "mode": 0o755} for name in ("home", "tmp", "work")}
    files = {"Makefile": cell.makefile, "protected.txt": PROTECTED,
             "matrix.json": metadata_bytes(cell), **dict(cell.created)}
    result.update({"work/" + name: regular_file(data) for name, data in files.items()})
    return result


def anchor_errors(cell: Cell, observation: conformance.Observation) -> list[str]:
    errors = []
    for field, expected in (("stdout", cell.stdout), ("stderr", cell.stderr), ("returncode", cell.status)):
        if getattr(observation, field) != expected:
            errors.append(f"authored {field} expectation failed")
    if observation.filesystem != expected_files(cell):
        errors.append("authored filesystem expectation failed")
    if observation.limit:
        errors.append(f"capture limit: {observation.limit}")
    if observation.leftover_processes:
        errors.append("leftover processes")
    return errors


def replay_arguments(cell: Cell, reference: Path, candidate: Path,
                     profile: Path, self_test: bool) -> list[str]:
    return ["python3", str(Path(__file__).resolve()), "--oracle", str(reference),
            "--oracle-profile", str(profile),
            *(["--self-test"] if self_test else ["--candidate", str(candidate)]),
            "--case", cell.case_id]


def save_failure(artifacts: Path, cell: Cell, case: conformance.Case, workspace: Path,
                 identities: dict[str, Any], observations: dict[str, conformance.Observation],
                 errors: list[str], replay: list[str]) -> Path:
    if set(observations) == {"oracle", "candidate"}:
        path = conformance.save_artifact(artifacts, case, workspace, identities,
                                        observations["oracle"], observations["candidate"], errors, "FAIL")
    else:
        artifacts.mkdir(parents=True, exist_ok=True)
        path = Path(tempfile.mkdtemp(prefix=cell.case_id + "-", dir=artifacts))
        shutil.copytree(case.path, path / "input", symlinks=True)
        report = {"status": "ERROR", "errors": errors, "identities": identities,
                  "argv": [str(workspace / "bin/make"), *case.args],
                  "cwd": str(workspace / "work"),
                  "environment": conformance.clean_environment(workspace),
                  "timeout_seconds": case.timeout, "observations": {}}
        for label, observed in observations.items():
            (path / (label + ".stdout")).write_bytes(observed.stdout)
            (path / (label + ".stderr")).write_bytes(observed.stderr)
            report["observations"][label] = conformance.observation_json(observed)
        (path / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    metadata = cell.metadata()
    metadata.update(replay_argv=replay, matrix_source_sha256=conformance.sha256_file(Path(__file__)))
    (path / "matrix.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    shutil.copyfile(Path(__file__), path / "diagnostic_matrix.py")
    return path


def run_campaign(*, cells: tuple[Cell, ...], reference: Path, candidate: Path,
                 identities: dict[str, Any], artifacts: Path, report_path: Path | None = None,
                 max_failures: int = DEFAULT_MAX_FAILURES, profile: Path = oracle.PROFILE,
                 self_test: bool = False, invocation: list[str] | None = None) -> int:
    validate_limit(max_failures)
    known = {cell.case_id: cell for cell in inventory()}
    if (not cells or len(cells) > len(known) or len({cell.case_id for cell in cells}) != len(cells)
            or any(known.get(cell.case_id) != cell for cell in cells)):
        raise conformance.HarnessError("campaign must select distinct original matrix cells")
    records = []
    failures = 0
    workload = hashlib.sha256()
    for cell in cells:
        workload.update(json.dumps(cell.metadata(), sort_keys=True).encode())
        workload.update(cell.makefile)
    with tempfile.TemporaryDirectory(prefix="rmake-diagnostic-matrix-") as temporary:
        root = Path(temporary)
        inputs, workspace = root / "inputs", root / "execution"
        inputs.mkdir()
        workspace.mkdir()
        for cell in cells:
            case = materialize(cell, inputs)
            replay = replay_arguments(cell, reference, candidate, profile, self_test)
            record = cell.metadata()
            record.update(status="PASS", argv=[str(workspace / "bin/make"), *case.args])
            records.append(record)
            observations = {}
            errors = []
            try:
                observations["oracle"] = conformance.execute(case, workspace, reference)
                observations["candidate"] = conformance.execute(case, workspace, candidate)
            except (conformance.HarnessError, OSError) as error:
                errors.append(str(error))
                record["status"] = "ERROR"
            else:
                errors += conformance.differences(observations["oracle"], observations["candidate"])
            for label, observed in observations.items():
                errors += [label + ": " + error for error in anchor_errors(cell, observed)]
            if errors:
                failures += 1
                if record["status"] != "ERROR":
                    record["status"] = "FAIL"
                saved = save_failure(artifacts, cell, case, workspace, identities,
                                     observations, errors, replay)
                record.update(errors=errors, artifact=str(saved), replay_argv=replay)
                print(f"{record['status']} {cell.case_id}: {', '.join(errors)}\n  evidence: {saved}", flush=True)
            if len(records) % 32 == 0 or len(records) == len(cells):
                print(f"Progress {len(records)}/{len(cells)} cells; failures={failures}", flush=True)
            if failures >= max_failures:
                print(f"Stopped after {failures} failures; {len(cells) - len(records)} cells unexecuted.", flush=True)
                break
    executed = len(records)
    report = {"matrix_version": MATRIX_VERSION,
              "matrix_source_sha256": conformance.sha256_file(Path(__file__)),
              "conformance_source_sha256": conformance.sha256_file(Path(conformance.__file__)),
              "python_version": sys.version, "invocation": invocation,
              "self_test": self_test, "oracle_profile": str(profile),
              "identities": identities, "inventory_cells": len(known),
              "cwd": str(workspace / "work"),
              "environment": conformance.clean_environment(workspace),
              "timeout_seconds": 5,
              "selected_cases": [cell.case_id for cell in cells],
              "workload_sha256": workload.hexdigest(), "max_failures": max_failures,
              "executed": executed, "passed": executed - failures,
              "failed": failures, "unexecuted": len(cells) - executed,
              "result": "FAIL" if failures else "PASS", "cells": records,
              "limitations": LIMITATIONS}
    if report_path is None:
        reports = artifacts / "campaigns"
        reports.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".json",
                                         prefix="diagnostic-matrix-", dir=reports, delete=False) as destination:
            json.dump(report, destination, indent=2, sort_keys=True)
            destination.write("\n")
            report_path = Path(destination.name)
    else:
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Diagnostic matrix v{MATRIX_VERSION}: cells={len(cells)}, executed={executed}, "
          f"passed={executed - failures}, failed={failures}, unexecuted={len(cells) - executed}")
    print(f"Campaign report: {report_path}")
    return int(bool(failures))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--self-test", action="store_true")
    mode.add_argument("--candidate", type=Path)
    parser.add_argument("--oracle", type=Path, default=conformance.ROOT / ".tools/oracle/bin/make")
    parser.add_argument("--oracle-profile", type=Path, default=oracle.PROFILE)
    parser.add_argument("--case", action="append", default=[], help="exact replay case ID; repeatable")
    parser.add_argument("--max-failures", type=int, default=DEFAULT_MAX_FAILURES)
    parser.add_argument("--artifacts", type=Path, default=conformance.ROOT / "artifacts/diagnostic-matrix")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args(argv)
    try:
        validate_limit(args.max_failures)
        cells = select_cells(args.case)
        profile_path = args.oracle_profile.expanduser().resolve()
        profile = oracle.verify(args.oracle.expanduser().resolve(), profile_path)
        reference_identity = conformance.executable_identity(args.oracle, oracle=True)
        reference_identity["profile"] = profile
        candidate_identity = (reference_identity if args.self_test else
                              conformance.executable_identity(args.candidate))
        print("Diagnostic matrix: strict observations of trusted fixtures; not a security sandbox.")
        print("Single invocations; timestamps are outside this gate.")
        return run_campaign(cells=cells, reference=Path(reference_identity["path"]),
                            candidate=Path(candidate_identity["path"]),
                            identities={"oracle": reference_identity, "candidate": candidate_identity},
                            artifacts=args.artifacts.expanduser().resolve(),
                            report_path=args.report.expanduser().resolve() if args.report else None,
                            max_failures=args.max_failures, profile=profile_path,
                            self_test=args.self_test,
                            invocation=["python3", str(Path(__file__).resolve()),
                                        *(sys.argv[1:] if argv is None else argv)])
    except (conformance.HarnessError, oracle.OracleError, OSError) as error:
        print(f"diagnostic-matrix: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
