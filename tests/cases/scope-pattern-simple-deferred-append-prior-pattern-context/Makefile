# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

%.t:X:=base
%.t:X+=$(info init=$@:$(X):$(Y))$(Y)
Y=late
all:a.t b.t
a.t:Y=local
b.t:Y=other
a.t b.t:;@printf '<%s>\n' '$(X)'
