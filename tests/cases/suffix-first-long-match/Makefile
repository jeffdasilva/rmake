# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake differential fixture.

.DEFAULT_GOAL := all
.SUFFIXES: .a.x .x
all: item.a.x
item.a.x:
	@printf '<%s>|<%s>|<%s>|<%s>\n' '$@' '$*' '$(*D)' '$(*F)'
