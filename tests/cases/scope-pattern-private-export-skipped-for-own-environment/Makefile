# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

export X=global
all:unexport X=ancestor
all:a.t
%.t:private export X=pattern
a.t:leaf
a.t leaf:;@printf '%s=<%s>/<%s>\n' '$@' '$(X)' "$${X-unset}"
