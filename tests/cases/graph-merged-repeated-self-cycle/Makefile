# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

.DEFAULT_GOAL := all
.PHONY: all left right a b c d e f x y z
all: all b all | x
	@printf 'auto=%s|first=%s|plus=%s|unique=%s|newer=%s|order=%s\n' '$@' '$<' '$+' '$^' '$?' '$|'
a b c d e f x y z:
	@printf 'visit=%s\n' '$@'
