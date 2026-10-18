# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

all: red.task
%.task: X = $@:$<
red.task: leaf
red.task leaf:
	@printf '%s=%s\n' '$@' '$(X)'
