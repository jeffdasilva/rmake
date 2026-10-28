# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
.DEFAULT_GOAL=all
-include generated.mk
generated.mk:;@printf "# generated\n" >$@
$(info R=[$(MAKE_RESTARTS)] A=[$(A)] C=[$(C)] N=[$(words $(file <seen))])
all:;@:
