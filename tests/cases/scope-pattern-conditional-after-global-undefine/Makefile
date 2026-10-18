# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X=first
%.t:X?=pattern
undefine X
all:a.t
a.t:;@printf '<%s>/%s/%s\n' '$(X)' '$(origin X)' '$(flavor X)'
