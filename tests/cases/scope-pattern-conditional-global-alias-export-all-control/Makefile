# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

.EXPORT_ALL_VARIABLES:
X = file
%.t: X ?= unused
a.t: ; @printf '%s/%s\n' '$(X)' "$${X-unset}"
