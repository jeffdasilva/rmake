# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

GNUMAKEFLAGS=-Iafter
all:;@$(eval MAKEFLAGS+=s)$(eval -include phase.mk)$(info D=[$(filter after,$(.INCLUDE_DIRS))] V=[$(VALUE)]):
