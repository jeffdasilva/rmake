# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

all: X = parent
all: red.task
%.task: private X = pattern
red.task: leaf
red.task leaf:
	@printf '%s=%s\n' '$@' '$(X)'
