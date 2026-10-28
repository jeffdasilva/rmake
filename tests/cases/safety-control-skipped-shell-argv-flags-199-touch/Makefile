# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
.ONESHELL:
define SHELL :=
./capture A
B
endef
define .SHELLFLAGS :=
one
two
endef
all:
	@printf body
.PHONY: all
