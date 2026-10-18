# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

@D = authored
@F := authored
@D += appended
@F += appended
.PHONY: dir/output
dir/output:
	@printf 'directory=%s/%s/%s\nfilename=%s/%s/%s\n' '$(@D)' '$(origin @D)' '$(flavor @D)' '$(@F)' '$(origin @F)' '$(flavor @F)'
