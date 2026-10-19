# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
all: inner1 inner2 inner3 inner4
inner1 inner2 inner3 inner4:
	@python3 worker.py enter $@
	@python3 worker.py leave $@
ifeq ($(FAIL),1)
all: bad
bad:;@exit 7
endif
