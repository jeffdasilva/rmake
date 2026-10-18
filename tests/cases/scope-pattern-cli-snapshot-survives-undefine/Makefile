# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

%.t:X=ignored
override undefine X
all:a.t
a.t:;@printf '<%s>/%s/%s\n' '$(X)' '$(origin X)' '$(flavor X)'
