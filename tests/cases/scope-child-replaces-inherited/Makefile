# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
.PHONY: all child leaf
all: X = outer
all: child
	@printf 'parent=%s\n' '$(X)'
child: X = inner
child: leaf
	@printf 'child=%s\n' '$(X)'
leaf:
	@printf 'leaf=%s\n' '$(X)'
