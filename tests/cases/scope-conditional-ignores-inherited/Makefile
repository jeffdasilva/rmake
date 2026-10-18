# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

.PHONY: all child leaf
all: X = parent
all: child
	@printf 'parent=%s\n' '$(X)'
child: X ?= child
child: leaf
	@printf 'child=%s\n' '$(X)'
leaf:
	@printf 'leaf=%s\n' '$(X)'
