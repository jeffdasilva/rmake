# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

all: left right
	@test "$$(cat count)" = x
	@printf '%s\n' once
left right: shared
shared:
	@printf x >> count
