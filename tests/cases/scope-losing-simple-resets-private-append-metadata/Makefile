# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = outer
all: override private X += tail
all: X := $(info rhs=<$(X)>)discarded
all: child
all child:
	@printf '%s=<%s>/<%s>/%s/%s\n' '$@' '$(X)' '$(value X)' '$(origin X)' '$(flavor X)'
