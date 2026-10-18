# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X=global
%.t:X+=one
a.%:X+=two
%.t:X+=three
all:a.t
a.t:;@printf '<%s>/<%s>/%s\n' '$(X)' '$(value X)' '$(flavor X)'
