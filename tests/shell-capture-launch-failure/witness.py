# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

import json
import os
from pathlib import Path
import sys
import time


def main(phase):
    ready = Path(phase + ".ready")
    proceed = Path(phase + ".go")
    draft = ready.with_suffix(".draft")
    draft.write_text(json.dumps({"pid": os.getpid(), "parent": os.getppid()}) + "\n")
    draft.rename(ready)
    deadline = time.monotonic() + 4.0
    while not proceed.exists():
        if time.monotonic() >= deadline:
            raise TimeoutError(phase)
        time.sleep(0.005)
    ready.unlink()
    proceed.unlink()
    if phase == "after-two":
        print("witness-ok")


if __name__ == "__main__":
    main(sys.argv[1])
