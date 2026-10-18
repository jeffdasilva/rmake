# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

PARSE := $(@D)/$(@F)/$(origin @)
.PHONY: dir/output
dir/output:
	@printf 'parse=%s\n' '$(PARSE)'
	@printf 'directory=%s/%s/%s\nfilename=%s/%s/%s\n' '$(@D)' '$(origin @D)' '$(flavor @D)' '$(@F)' '$(origin @F)' '$(flavor @F)'
