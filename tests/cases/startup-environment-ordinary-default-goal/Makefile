# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

$(info CURDIR=<$(if $(filter $(abspath .),$(CURDIR)),cwd,$(CURDIR))>/$(origin CURDIR)/$(flavor CURDIR))
$(foreach N,MAKEFILE_LIST SUFFIXES MAKE_VERSION MAKE MAKE_COMMAND MAKECMDGOALS,$(info $(N)=<$($(N))>/$(origin $(N))/$(flavor $(N))))
all: ; @:
