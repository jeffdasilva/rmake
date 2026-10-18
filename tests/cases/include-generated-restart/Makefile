# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva

.DEFAULT_GOAL := all
$(info pass=$(if $(MAKE_RESTARTS),$(MAKE_RESTARTS),0))
include generated.mk
all:
	@printf '%s\n' '$(TOKEN)'
generated.mk:
	@printf '%s\n' 'TOKEN = generated' > $@
