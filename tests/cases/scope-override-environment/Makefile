# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

.PHONY: all child
all: override X = forced
all: child
	@printf 'parent=%s/%s\n' '$(X)' '$(origin X)'
child:
	@printf 'child=%s/%s\n' '$(X)' '$(origin X)'
