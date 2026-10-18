# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
all: red blue
red: export X = red
red blue:
	@printf '%s=%s/%s\n' '$@' '$(X)' "$${X-unset}"
