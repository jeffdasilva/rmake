# SPDX-License-Identifier: GPL-3.0-or-later
# Original rmake differential fixture.

all: item.o item.yl item.lm
	@printf "<%s>|%s|%s\n" "$(SUFFIXES)" "$(origin SUFFIXES)" "$(flavor SUFFIXES)"
item.o item.yl item.lm:
	@printf '<%s>|<%s>|<%s>|<%s>\n' '$@' '$*' '$(*D)' '$(*F)'
