# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
.PHONY: left right shared
left: X = left
right: X = right
left right: shared
	@printf '%s=%s\n' '$@' '$(X)'
shared:
	@printf 'shared=%s\n' '$(X)'
