# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
MAKE_RESTARTS=0
$(info R=[$(MAKE_RESTARTS)] N=[$(words $(file <seen))])
.DEFAULT_GOAL=all
include generated.mk
generated.mk:;@printf "E\n" >>seen;printf "# generated\n" >$@
all:;@:
