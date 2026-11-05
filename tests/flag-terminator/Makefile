# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

$(info pre|$(MAKECMDGOALS)|$(-X))
# CASE-DIRECTIVES
$(info post|$(MAKECMDGOALS)|$(-X))
all -s --trace -- explicit selected:
	printf '%s\n' 'recipe|$@|$(MAKEFLAGS)|$(-X)' > '$@.out'
