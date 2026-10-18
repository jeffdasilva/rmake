# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

%.t: X := base
%.t: X += $(info activate)tail
all:left right
left right:a.t
a.t:;@printf 'recipe=%s\n' '$(X)'
