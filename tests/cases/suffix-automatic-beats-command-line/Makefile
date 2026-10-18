# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake differential fixture.

.DEFAULT_GOAL := all
.SUFFIXES: .o
all: item.o
item.o:
	@printf '<%s>|<%s>|<%s>|<%s>\n' '$@' '$*' '$(*D)' '$(*F)'
