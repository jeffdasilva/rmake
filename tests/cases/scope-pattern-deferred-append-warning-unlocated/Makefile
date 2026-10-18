# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

%.t: X := base
%.t: X += $(warning deferred)tail
all: a.t
a.t: ; @printf '%s\n' '$(X)'
