# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

Y = early
.PHONY: all child
all: X := first
all: X += $(Y)
Y = late
all: child
	@printf 'parent=%s/%s\n' '$(X)' '$(flavor X)'
child:
	@printf 'child=%s/%s\n' '$(X)' '$(flavor X)'
