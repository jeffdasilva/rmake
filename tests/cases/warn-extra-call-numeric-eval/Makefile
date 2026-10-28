# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
BODY=$(eval RESULT := $(call INNER,$(1)))$(RESULT)
INNER=$(1)/$(2)
all:;@$(info result=[$(let 2,OUTER,$(call BODY,x))]):
