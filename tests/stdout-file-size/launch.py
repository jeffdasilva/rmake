# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Run make with a capped inherited output file."""
import os
import resource
import signal
import sys

mode, limit = sys.argv[1:3]
flag = os.O_APPEND if mode.endswith('-append') else os.O_TRUNC
descriptor = os.open('capture', os.O_WRONLY | os.O_CREAT | flag, 0o644)
os.dup2(descriptor, 2 if mode.startswith('stderr') else 1)
os.close(descriptor)
signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
resource.setrlimit(resource.RLIMIT_FSIZE, (int(limit), int(limit)))
os.execv(sys.argv[3], sys.argv[3:])
