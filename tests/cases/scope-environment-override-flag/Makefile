# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
.PHONY: all child
all: X = target
all: child
	@printf 'parent=%s/%s\n' '$(X)' '$(origin X)'
child:
	@printf 'child=%s/%s\n' '$(X)' '$(origin X)'
