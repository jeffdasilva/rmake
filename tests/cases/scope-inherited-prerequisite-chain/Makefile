# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
.PHONY: all parent child leaf other
all: parent other
parent: X = parent
parent: child
	@printf 'parent=%s\n' '$(X)'
child: leaf
	@printf 'child=%s\n' '$(X)'
leaf:
	@printf 'leaf=%s\n' '$(X)'
other:
	@printf 'other=%s\n' '$(X)'
