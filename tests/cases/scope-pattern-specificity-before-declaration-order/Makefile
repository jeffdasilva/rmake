# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X=global
ab%.t:X+=specific-one
%.t:X+=general
ab%.t:X+=specific-two
all:abred.t
abred.t:;@printf '<%s>/<%s>\n' '$(X)' '$(value X)'
