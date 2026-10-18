# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = base
%: export X +=
%1: private X := frozen
all: n1
n1: ; @printf '<%s>/<%s>/%s/<%s>\n' '$(X)' '$(value X)' '$(flavor X)' "$${X-unset}"
