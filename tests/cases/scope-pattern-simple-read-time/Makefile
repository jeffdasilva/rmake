# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = old
all: red.task
%.task: Y := $(X)
X = new
red.task: X = target
red.task:
	@printf '%s/%s\n' '$(X)' '$(Y)'
