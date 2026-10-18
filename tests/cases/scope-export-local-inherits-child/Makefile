# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
all: export X = parent
all: child other
all child other:
	@printf '%s=%s/%s\n' '$@' '$(X)' "$${X-unset}"
