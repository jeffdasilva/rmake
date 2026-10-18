# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

.PHONY: all a p/b p/c order
all: a p/b a p/c | order p/b
	@printf 'first=<%s>/<%s>\nunique=<%s>/<%s>\nrepeated=<%s>/<%s>\nnewer=<%s>/<%s>\norder=<%s>\n' '$(<D)' '$(<F)' '$(^D)' '$(^F)' '$(+D)' '$(+F)' '$(?D)' '$(?F)' '$|'
a p/b p/c order:
	@:
