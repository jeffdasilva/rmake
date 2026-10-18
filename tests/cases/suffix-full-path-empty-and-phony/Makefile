# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake differential fixture.

.DEFAULT_GOAL := all
.SUFFIXES: .o
.PHONY: item.o dir/item.o .o dir/.o
all: item.o dir/item.o .o dir/.o
item.o dir/item.o .o dir/.o:
	@printf '<%s>|<%s>|<%s>|<%s>\n' '$@' '$*' '$(*D)' '$(*F)'
