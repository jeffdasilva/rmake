import itertools
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile

WORKER = Path(__file__).with_name("worker.mk").read_bytes()
TEMP_NAME = re.compile(rb"relative///Gm[A-Za-z0-9]{6}(?=:)")
TEMP_ERROR = b"make: cannot create temporary file relative///Gm<TEMP>: No such file or directory\n"
SUPPRESSION = b"make: cannot open output-sync lock file, suppressing output-sync.\n"


def reader_events(reader, phase, working, jobs):
    if reader == "warning":
        entry = (True, b"../worker.mk:2: READER-WARNING:" + phase + b"\n")
    elif reader == "shell":
        entry = (True, b"READER-SHELL:" + phase + b"\n")
    else:
        entry = (False, b"READER-OUT:" + phase + b"\n")
    if working:
        return [entry]
    events = [(True, TEMP_ERROR), (True, SUPPRESSION)]
    events.append((True, SUPPRESSION) if reader == "warning" else entry)
    if jobs == 2:
        events.extend([(True, TEMP_ERROR), (True, SUPPRESSION), (True, SUPPRESSION)])
    return events


def expected_streams(reader, repair, jobs, merged):
    events = reader_events(reader, b"", False, jobs)
    events.extend([(False, b"REMAKE:\n"), (True, b"REMAKE-ERR\n"), (False, b"REMAKE-NEXT\n")])
    events.extend(reader_events(reader, b"1", repair, jobs))
    events.extend([
        (False, b"ORDINARY:1:loaded\n"),
        (True, b"ORDINARY-ERR\n"),
        (False, b"ORDINARY-NEXT\n"),
    ])
    if merged:
        return b"".join(message for _, message in events), b""
    return tuple(b"".join(message for is_error, message in events if is_error == error)
                 for error in (False, True))


profiles = itertools.product(("line", "target"), (1, 2), ("info", "warning", "shell"),
                             (False, True), (False, True), (False, True))
count = 0
for mode, jobs, reader, repair, optional, merged in profiles:
    with tempfile.TemporaryDirectory(prefix="reader-restart-", dir=".") as temporary:
        root = Path(temporary)
        (root / "relative").mkdir()
        shifted = root / "shifted"
        shifted.mkdir()
        (root / "worker.mk").write_bytes(WORKER)
        environment = {key: value for key, value in os.environ.items()
                       if key not in ("MAKEFLAGS", "GNUMAKEFLAGS", "MFLAGS", "MAKELEVEL", "MAKE_RESTARTS")}
        environment.update(MAKE_TMPDIR="relative///", READER=reader,
                           REPAIR="yes" if repair else "", OPTIONAL="yes" if optional else "")
        command = [sys.argv[1], "-rR", "--no-print-directory", "--jobserver-style=pipe",
                   f"-j{jobs}", f"-O{mode}", "-C", "shifted", "-f", "../worker.mk", "all"]
        result = subprocess.run(command, cwd=root, env=environment, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT if merged else subprocess.PIPE,
                                timeout=3, umask=0o022)
        observed = tuple(TEMP_NAME.sub(b"relative///Gm<TEMP>", data)
                         for data in (result.stdout, result.stderr or b""))
        expected = expected_streams(reader, repair, jobs, merged)
        assert (result.returncode, observed) == (0, expected), (
            mode, jobs, reader, repair, optional, merged, result.returncode, observed, expected
        )
        generated = shifted / "generated.mk"
        assert generated.read_bytes() == b"VALUE := loaded\n"
        assert stat.S_IMODE(generated.stat().st_mode) == 0o600
        assert {path.name for path in shifted.iterdir()} == (
            {"generated.mk", "relative"} if repair else {"generated.mk"}
        )
        if repair:
            repaired = shifted / "relative"
            assert stat.S_IMODE(repaired.stat().st_mode) == 0o700
            assert list(repaired.iterdir()) == []
        assert list((root / "relative").iterdir()) == []
        count += 1
print(f"verified {count} reader restart profiles")
