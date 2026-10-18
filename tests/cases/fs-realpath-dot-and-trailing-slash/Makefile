# Copyright 2026 Jeff DaSilva
# SPDX-License-Identifier: GPL-3.0-or-later

all:;@printf '%s/<%s>\n' '$(if $(filter $(CURDIR),$(realpath . ./ dir/..)),cwd,wrong)' '$(patsubst $(CURDIR)/%,%,$(realpath dir///))'
