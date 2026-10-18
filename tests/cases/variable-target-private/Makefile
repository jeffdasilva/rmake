# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

MODE = global
all: private MODE = local
all: child
	@printf '%s\n' 'parent=$(MODE)'
child:
	@printf '%s\n' 'child=$(MODE)'
