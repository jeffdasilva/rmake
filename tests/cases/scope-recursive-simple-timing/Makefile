# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

BASE = early
.PHONY: all
all: SIMPLE := $(BASE)
all: RECURSIVE = $(BASE)
BASE = late
all: BASE = target
all:
	@printf 'simple=%s/%s\nrecursive=%s/%s\nbase=%s\n' '$(SIMPLE)' '$(flavor SIMPLE)' '$(RECURSIVE)' '$(flavor RECURSIVE)' '$(BASE)'
