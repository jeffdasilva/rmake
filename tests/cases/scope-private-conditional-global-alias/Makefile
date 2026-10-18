# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

X = global
all: parent unrelated
parent: private X ?= ignored
$(info parse=$(X)/$(origin X))
parent unrelated:
	@printf '%s=%s/%s\n' '$@' '$(X)' '$(origin X)'
