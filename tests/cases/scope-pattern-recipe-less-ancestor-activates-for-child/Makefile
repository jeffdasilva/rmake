# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

%.t: X := base
%.t: X += $(info activate)tail
all:a.t
a.t:child
child:;@printf 'child=%s\n' '$(X)'
