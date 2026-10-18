# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

all: export X = parent
all: child
child: X = child
child:
	@printf '%s/%s\n' '$(X)' "$${X-unset}"
