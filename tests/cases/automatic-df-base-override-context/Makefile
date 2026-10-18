# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

override @ = parse/forced
PARSE := $(@D)/$(@F)/$(origin @)
.PHONY: dir/output
dir/output:
	@printf 'parse=%s\n' '$(PARSE)'
	@printf 'recipe=%s/%s/%s/%s\n' '$@' '$(@D)' '$(@F)' '$(origin @)'
