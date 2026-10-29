# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

$(info N=[$(words $(file <observed))] D=[$(lastword $(file <observed))])
include generated.mk
generated.mk:;@printf 'X=ready\n' > $@
all:;@:
