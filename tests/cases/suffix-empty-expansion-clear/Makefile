# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake differential fixture.

.DEFAULT_GOAL := all
.SUFFIXES: .x
.SUFFIXES: $(UNDEFINED)
all: item.x
item.x:
	@printf '<%s>|<%s>|<%s>|<%s>\n' '$@' '$*' '$(*D)' '$(*F)'
