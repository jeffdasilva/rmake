# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = first
all: X ?= ignored
X = later
all:
	@printf '%s\n' '$(X)'
