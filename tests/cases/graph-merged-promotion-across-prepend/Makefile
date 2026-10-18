# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

.DEFAULT_GOAL := all
.PHONY: all left right a b c d e f x y z
all: b | a
all: | a c
	@printf 'auto=%s|first=%s|plus=%s|unique=%s|newer=%s|order=%s\n' '$@' '$<' '$+' '$^' '$?' '$|'
all: d a
a b c d e f x y z:
	@printf 'visit=%s\n' '$@'
