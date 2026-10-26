# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
include config.mk
unexport HOME
out: ~/*.in | ~/order
	@python3 build.py '$@' '$^' '$?' '$(HOME)'
verify: out
	@cat out
