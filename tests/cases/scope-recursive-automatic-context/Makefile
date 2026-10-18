# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

.PHONY: all child
all: X = $@/$<
all: child
	@printf 'parent=%s\n' '$(X)'
child:
	@printf 'child=%s\n' '$(X)'
