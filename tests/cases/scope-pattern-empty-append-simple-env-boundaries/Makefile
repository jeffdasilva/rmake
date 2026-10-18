# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = base
n2: export X +=
%: X +=
%1: private X := $@
all: n2
n2: n1
n1: ; @printf '<%s>/<%s>/%s/<%s>\n' '$(X)' '$(value X)' '$(flavor X)' "$${X-unset}"
