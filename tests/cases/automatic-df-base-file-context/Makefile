# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

@ = parse/input
CAPTURED := $(@D)
DEFERRED = $(@D)
PARSE := $(@D)/$(@F)/$(origin @)
.PHONY: dir/output
dir/output:
	@printf 'parse=%s\n' '$(PARSE)'
	@printf 'recipe=%s/%s/%s\ncaptured=%s\ndeferred=%s\n' '$(@D)' '$(@F)' '$(origin @)' '$(CAPTURED)' '$(DEFERRED)'
