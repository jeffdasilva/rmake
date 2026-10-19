# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
all: recurse outer1 outer2
	@test "$$(cat maximum)" = 3 && test "$$(cat active)" = 0
	@echo complete
recurse:
	+@$(CHILD) -f child.mk --no-print-directory
	@touch recursive.done
outer1 outer2:
	@python3 worker.py enter $@
	@python3 worker.py leave $@
