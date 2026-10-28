# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
BODY=$(call INNER,$(1))
INNER=$(1)/$(2)
export OBSERVED_EXPRESSION = $(call BODY,x,y)
all:;@printf "result=[%s]\n" "$$OBSERVED_EXPRESSION"
