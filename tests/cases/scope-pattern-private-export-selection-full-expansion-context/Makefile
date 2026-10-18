# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

export X = global-$(Z)
export Y = $(X)
Z = globalz
all: a.t
%.t: private export X = pattern
%.t: Z = targetz
a.t: ; @printf '%s/%s/%s/%s\n' '$(X)' '$(Y)' "$$X" "$$Y"
