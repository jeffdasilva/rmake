# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

%.t:X:=$$(KEEP)
%.t:X+=$(info append=$(X))$$(LATE)
all:a.t
a.t:;@printf '<%s>/<%s>/%s\n' '$(X)' '$(X)' '$(flavor X)'
