# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
all: child
all: private X = local
all: X ?= ignored
all:
	@printf 'all=%s\n' '$(X)'
child:
	@printf 'child=%s\n' '$(X)'
