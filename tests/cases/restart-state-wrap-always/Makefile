# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
$(info R=[$(MAKE_RESTARTS)] A=[$(A)] N=[$(words $(file <seen))])
.DEFAULT_GOAL=all
-include generated.mk
generated.mk:;@python3 regen.py
all:;@:
