# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake differential fixture.

.DEFAULT_GOAL := all
.SUFFIXES: .x
.SUFFIXES: | .a.x .y
all: item.a.x item.y
item.a.x item.y:
	@printf '<%s>|<%s>|<%s>|<%s>\n' '$@' '$*' '$(*D)' '$(*F)'
