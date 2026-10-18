# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

all:a%b.t axb.t
a\%b.t:X=literal
%:Y=pattern
a\%b.t axb.t:;@printf '%s=<%s>/<%s>\n' '$@' '$(X)' '$(Y)'
