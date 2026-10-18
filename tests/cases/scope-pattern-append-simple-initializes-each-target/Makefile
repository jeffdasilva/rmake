# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

K = old
%.task: X := base-$(K)
%.task: X += $(info append=$@:$(K))tail-$(K)
K = new
all: red.task blue.task
red.task: K = red
blue.task: K = blue
red.task blue.task:
	@printf '%s=%s/%s/<%s>\n' '$@' '$(X)' '$(flavor X)' '$(value X)'
