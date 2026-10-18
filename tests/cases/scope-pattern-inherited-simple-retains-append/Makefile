# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = base
%: private X += suffix
a%: X := frozen
all: n2
n2: ; @printf '<%s>/<%s>/%s/<%s>\n' '$(X)' '$(value X)' '$(flavor X)' "$${X-unset}"
