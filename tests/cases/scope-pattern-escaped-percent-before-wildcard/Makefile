# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

all:a%b-red.t axb-red.t
a\%b-%.t:X=match
%:Y=fallback
a\%b-red.t axb-red.t:;@printf '%s=<%s>/<%s>\n' '$@' '$(X)' '$(Y)'
