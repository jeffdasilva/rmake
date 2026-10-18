# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
.PHONY: all target other
all: target other
target: X = local
target: Y := $(X)
target:
	@printf 'target=%s/%s\n' '$(X)' '$(Y)'
other:
	@printf 'other=%s/%s\n' '$(X)' '$(Y)'
