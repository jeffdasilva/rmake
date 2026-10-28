# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
override export CC=OVERRIDE
MAKEFLAGS += -R
all:;@$(eval CC=AFTER)printf "env=[%s]\n" "$$CC"
