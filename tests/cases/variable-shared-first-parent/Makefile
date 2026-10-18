# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

all: first second
first: MODE = left
second: MODE = right
first second: shared
shared:
	@printf '%s\n' '$(MODE)'
