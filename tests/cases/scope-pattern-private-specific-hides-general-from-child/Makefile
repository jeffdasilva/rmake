# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X=global
%.t:X=general
a.%:private X=specific
all:a.t
a.t:leaf
a.t leaf:;@printf '%s=<%s>\n' '$@' '$(X)'
