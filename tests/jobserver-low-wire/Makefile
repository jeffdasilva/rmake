# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

all:
	+@$(file >parent-auth,$(filter --jobserver-auth=%,$(MAKEFLAGS)))$(CHILD) -f Child.mk child
