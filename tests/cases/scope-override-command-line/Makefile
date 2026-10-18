# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

.PHONY: all parent child other
all: parent other
parent: override X = forced
parent: child
	@printf 'parent=%s/%s\n' '$(X)' '$(origin X)'
child:
	@printf 'child=%s/%s\n' '$(X)' '$(origin X)'
other:
	@printf 'other=%s/%s\n' '$(X)' '$(origin X)'
