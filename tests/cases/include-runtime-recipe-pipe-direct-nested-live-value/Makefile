# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

all:;@$(eval MAKEFLAGS = -rR --no-print-directory -I- -Ia-$$$$(if\ $$$$(DONE),,$$$$(eval\ DONE:=1)$$$$(eval\ MAKEFLAGS+=-Iinner)) -Ib-$$$$(eval\ -include\ probe.mk)$$$$(file\ >>observed,$$$$(VALUE)))$(info SEEN=[$(word 1,$(file <observed))]/[$(word 2,$(file <observed))] VALUE=[$(VALUE)])$(eval undefine AFTER)$(eval -include inner-only.mk)$(info AFTER=[$(AFTER)])$(info FLAG=[$(filter -Iinner,$(MAKEFLAGS))])$(info DIR=[$(.INCLUDE_DIRS)]):
