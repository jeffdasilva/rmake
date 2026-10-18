# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

export X = global
all: private X = parent
all: child
all child:
	@printf '%s=%s/%s\n' '$@' '$(X)' "$${X-unset}"
