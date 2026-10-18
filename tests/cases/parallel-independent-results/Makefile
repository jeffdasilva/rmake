# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

all: left right
	@test "$$(cat left)" = left
	@test "$$(cat right)" = right
	@printf '%s\n' complete
left right:
	@printf '%s\n' '$@' > $@
