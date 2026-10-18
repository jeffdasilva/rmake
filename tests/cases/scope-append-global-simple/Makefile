# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

K = early
X := base-$(K)
.PHONY: all
all: X += tail-$(K)
K = late
all:
	@printf 'expanded=%s\nraw=%s\nflavor=%s\n' '$(X)' '$(value X)' '$(flavor X)'
