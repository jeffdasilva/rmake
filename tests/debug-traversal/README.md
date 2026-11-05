# Implicit search debug output

These cases compare GNU Make's deterministic search and cleanup transcript
against rmake. Each test uses an original small Makefile, fixed input times,
and the same raw invocation path. The models independently require selected
debug events, exit status, and resulting files. The differential gate checks
all stream bytes and the complete fixture filesystem against two verified GNU
4.4.1 runs.

Search messages reuse the directory and VPATH results already observed by the
runtime. The ordinary search makes no extra filesystem call or rule-label
allocation. The GNU 4.4.1 source files `implicit.c` and `file.c` were inspected
for observable event boundaries; the implementation and fixtures are original.
