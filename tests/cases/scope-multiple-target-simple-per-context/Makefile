# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

all: red blue
red: X = red
blue: X = blue
red blue: Y := $(info assign=$(X))value-$(X)
red blue:
	@printf '%s=%s\n' '$@' '$(Y)'
