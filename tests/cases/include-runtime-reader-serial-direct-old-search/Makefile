# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

MAKEFLAGS = -rR --no-print-directory -I- -Inew-$$(eval\ -include\ probe.mk)
$(info META=[$(VALUE)])
include probe.mk
$(info AFTER=[$(VALUE)])
all:;@:
