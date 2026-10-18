# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
EMPTY =
.PHONY: all child
all: X ?= replacement
all: EMPTY ?= replacement
all: NEW ?= assigned
all: child
	@printf 'parent=<%s>/<%s>/<%s>\n' '$(X)' '$(EMPTY)' '$(NEW)'
child:
	@printf 'child=<%s>/<%s>/<%s>\n' '$(X)' '$(EMPTY)' '$(NEW)'
