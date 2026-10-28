# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
.DEFAULT_GOAL=all
-include first.mk
first.mk:;@printf '%s\n' '-include second.mk' >$@
second.mk:;@printf "# generated\n" >$@
$(info [$(MAKE_RESTARTS)]/[$(SNAP)]/[$(words $(file <seen))])
all:;@:
