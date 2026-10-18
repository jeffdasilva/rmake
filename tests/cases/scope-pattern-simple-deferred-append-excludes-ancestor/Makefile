# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

K=global
all:K=ancestor
all:a.t
%.t:X:=early
%.t:X+=$(K)
a.t:;@printf '<%s>\n' '$(X)'
