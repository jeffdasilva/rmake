# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
.DEFAULT_GOAL=all
-include generated.mk
generated.mk:;@printf '$$(error SECOND)\n' >early.mk; printf "# generated\n" >$@
$(info [$(MAKE_RESTARTS)])
all:;@:
