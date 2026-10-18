# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

export X=global
all:a.t
%.t:unexport X=pattern
a.t:leaf
leaf:X=child
a.t leaf:;@printf '%s=<%s>/<%s>\n' '$@' '$(X)' "$${X-unset}"
