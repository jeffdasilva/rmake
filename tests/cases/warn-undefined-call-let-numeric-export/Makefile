# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
BODY=$(1)/$(2)
export OBSERVED_EXPRESSION = $(let 2,OUTER,$(call BODY,x))
all:;@printf "result=[%s]\n" "$$OBSERVED_EXPRESSION"
