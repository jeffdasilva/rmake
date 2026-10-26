# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
.SECONDEXPANSION:
PATTERN=inputs/*.in
out\ file: $$(PATTERN)
	@python3 build.py '$@' '$^' '$?'
verify: out\ file
	@cat 'out file'
