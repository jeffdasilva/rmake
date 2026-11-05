# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Pin a known mask before the observed program image."""
import os
import resource
import signal
import sys

selected = {signal.SIGUSR2, signal.SIGTERM} if sys.argv[1] == 'blocked' else set()
signal.pthread_sigmask(signal.SIG_SETMASK, selected)
nofile = int(sys.argv[2])
disposition = {'default': signal.SIG_DFL, 'ignore': signal.SIG_IGN}.get(sys.argv[3])
if disposition is not None:
    signal.signal(signal.SIGPIPE, disposition)
resource.setrlimit(resource.RLIMIT_NOFILE, (nofile, nofile))
os.environ['RM_MASK_PID'] = str(os.getpid())
os.environ['LD_PRELOAD'] = os.path.abspath('loader.so')
os.execv(sys.argv[4], sys.argv[4:])
