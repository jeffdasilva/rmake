# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

export X = global
all: X := parent
n%: private X ?= ignored
all: anchor n0
anchor: ; @printf 'anchor\n'
n0: ; @printf '<%s>/<%s>\n' '$(X)' "$${X-unset}"
