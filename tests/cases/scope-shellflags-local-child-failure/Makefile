# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

all: .SHELLFLAGS = -ec
all: child
all: ; @printf 'unreachable parent\n'
child:
	@false; printf 'unreachable child\n'
