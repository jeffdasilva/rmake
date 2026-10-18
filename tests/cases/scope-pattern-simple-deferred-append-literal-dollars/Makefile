# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

K=$$literal
%.t:X:=$$(KEEP)
%.t:X+=$$(LATE)-$(K)
all:a.t
a.t:;@printf '<%s>/<%s>/%s\n' '$(X)' '$(value X)' '$(flavor X)'
