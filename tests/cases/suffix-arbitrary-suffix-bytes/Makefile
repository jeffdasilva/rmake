# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake differential fixture.

.DEFAULT_GOAL := all
.SUFFIXES: x /item.o
all: itemx item.x dir/item.o
itemx item.x dir/item.o:
	@printf '<%s>|<%s>|<%s>|<%s>\n' '$@' '$*' '$(*D)' '$(*F)'
