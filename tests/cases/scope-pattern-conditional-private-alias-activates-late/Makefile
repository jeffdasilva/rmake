# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X=global
all:before a.t after
before after:;@printf '%s=<%s>/%s\n' '$@' '$(X)' '$(origin X)'
%.t:private X?=ignored
a.t:;@printf '%s=<%s>/%s\n' '$@' '$(X)' '$(origin X)'
