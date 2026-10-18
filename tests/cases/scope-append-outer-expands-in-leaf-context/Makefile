# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

K = global
X = base-$(K)
all: K = parent
all: X += parent-$(K)
all: child
child: K = child
child: X += child-$(K)
child:
	@printf '%s\n' '$(X)'
