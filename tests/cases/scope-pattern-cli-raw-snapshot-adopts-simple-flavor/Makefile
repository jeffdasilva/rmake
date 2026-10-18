# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

Y=early
%.t:X:=ignored
Y=later
all:a.t
a.t:;@printf '<%s>/<%s>/%s/%s\n' '$(X)' '$(value X)' '$(origin X)' '$(flavor X)'
