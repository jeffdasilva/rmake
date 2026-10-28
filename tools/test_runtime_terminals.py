# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Reproduce completion between an empty select result and process.poll()."""
import os
from pathlib import Path
import pty
import tempfile
import unittest
from unittest import mock
import runtime_terminals as terminals

FAKE = b'''#!/usr/bin/python3
import os
out=os.ttyname(1)
err=out if os.isatty(2) else 'unset'
text='termout-origin=default\\ntermerr-origin='+('default' if os.isatty(2) else 'undefined')+'\\n'
for fd,name in [(0,'stdin'),(1,'stdout'),(2,'stderr')]:text+=name+'-tty='+str(int(os.isatty(fd)))+'\\n'
text+='MAKE_TERMOUT=['+out+']\\nMAKE_TERMERR=['+err+']\\n'
os.write(1,text.encode())
os.write(2,b'child-stderr\\n')
'''


class TerminalCompletionTests(unittest.TestCase):
    def check_completion_race(self, merged):
        with tempfile.TemporaryDirectory(prefix='rmake-pty-regression-') as temporary:
            path = Path(temporary)
            binary = path / 'producer'
            binary.write_bytes(FAKE)
            binary.chmod(0o700)
            fixture = path / 'fixture'
            fixture.mkdir()
            cell = terminals.Cell('none', 1, False, merged, False)
            (fixture / 'Makefile').write_bytes(cell.source)
            (fixture / 'helper.py').write_bytes(terminals.HELPER)
            root = path / 'run'
            root.mkdir()
            master, slave = pty.openpty()
            os.set_blocking(master, False)
            popen = terminals.subprocess.Popen
            select = terminals.select.select
            try:
                # Reuse the same PTY as the real differential runner. Each
                # observation must own all of its bytes, including the tail.
                for _ in range(2):
                    processes = []
                    forced = []

                    def start(*args, **kwargs):
                        process = popen(*args, **kwargs)
                        processes.append(process)
                        return process

                    def finish_after_empty_ready(*args, **kwargs):
                        if not forced:
                            forced.append(True)
                            processes[0].wait(timeout=2)
                            return [], [], []
                        return select(*args, **kwargs)

                    with mock.patch.object(terminals.subprocess, 'Popen', side_effect=start), \
                         mock.patch.object(terminals.select, 'select', side_effect=finish_after_empty_ready):
                        observed, errors = terminals.execute(cell, root, fixture, binary, master, slave)
                    self.assertEqual(errors, [])
                    self.assertEqual((observed.stdout, observed.stderr), cell.expected(os.ttyname(slave)))
                    self.assertEqual(observed.returncode, 0)
                    self.assertIsNone(observed.limit)
            finally:
                os.close(master)
                os.close(slave)

    def test_separate_pipe_and_pty_keep_completion_tail(self):
        self.check_completion_race(False)

    def test_merged_pty_keeps_completion_tail(self):
        self.check_completion_race(True)


if __name__ == '__main__':
    unittest.main()
