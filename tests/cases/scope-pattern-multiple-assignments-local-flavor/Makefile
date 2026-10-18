# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

K = old
all: red.task
%.task: X := base-$(K)
%.task: X += tail-$(K)
K = new
red.task:
	@printf '%s/%s\n' '$(X)' '$(flavor X)'
