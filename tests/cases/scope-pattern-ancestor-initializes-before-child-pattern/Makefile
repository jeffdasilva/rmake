# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

parent.%: X := parent
parent.%: X += $(info parent-activate)tail
child.%: Y := child
child.%: Y += $(info child-activate)tail
all: parent.t
parent.t: child.t
parent.t child.t: ; @printf '%s=%s/%s\n' '$@' '$(X)' '$(Y)'
