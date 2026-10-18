# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

%.t: X := base
%.t: X += $(file >>record,activate)tail
all: first.t second.t
first.t second.t:;@printf '%s\n' '$(X)'
