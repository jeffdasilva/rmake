# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Set an exact descriptor limit and explicit inherited signals before exec."""
import os
import resource
import signal
import sys

limit = int(sys.argv[1])
signal.signal(signal.SIGPIPE, signal.SIG_IGN if sys.argv[2] == 'ignore' else signal.SIG_DFL)
if sys.argv[3] == 'block':
    signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGUSR2})
resource.setrlimit(resource.RLIMIT_NOFILE, (limit, limit))
os.execv(sys.argv[4], sys.argv[4:])
