# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
out\ file: in\ file | order\ file
	@python3 build.py '$@' '$<' '$?'
verify: out\ file
	@cat 'out file'
