# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

all:dir/red.t
%.t:X=wild
red.t:Y=literal
r%.t:Z=prefix
dir/red.t:;@printf '<%s>/<%s>/<%s>\n' '$(X)' '$(Y)' '$(Z)'
