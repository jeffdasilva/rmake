# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

%.t:private X=local
%.t:X?=ignored
all:a.t
a.t:leaf
a.t leaf:;@printf '%s=<%s>\n' '$@' '$(X)'
