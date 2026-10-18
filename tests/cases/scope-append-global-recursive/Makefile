# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global-$(LATE)
LATE = before
.PHONY: all child
all: X += local-$(LATE)
all: LATE = target
all: child
	@printf 'parent=%s\n' '$(X)'
child:
	@printf 'child=%s\n' '$(X)'
