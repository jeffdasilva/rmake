# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
all: libred.task
lib%.task: X += specific
%.task: X += general
libred.task: X += explicit
libred.task:
	@printf '%s/<%s>\n' '$(X)' '$(value X)'
