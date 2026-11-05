#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Execute a build or self-host command using only verified Guile SDK settings."""
import argparse
import os
from pathlib import Path
from guile_frontend import ENVIRONMENT, read_profile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--profile", type=Path, required=True)
parser.add_argument("command", nargs=argparse.REMAINDER)
args = parser.parse_args()
command = args.command[1:] if args.command[:1] == ["--"] else args.command
if not command:
    parser.error("provide a command after --")
profile = read_profile(args.profile)
environment = os.environ.copy()
for key in (*ENVIRONMENT, "RMAKE_GUILE_PREFIX"):
    environment[key] = profile["environment"][key]
os.execvpe(command[0], command, environment)
