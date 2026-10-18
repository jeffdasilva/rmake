# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

%.t:X=general
a.%:X?=specific
all:a.t
a.t:;@printf '<%s>/%s/%s\n' '$(X)' '$(origin X)' '$(flavor X)'
