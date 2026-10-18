# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

undefine @D
override undefine @F
.PHONY: dir/output
dir/output:
	@printf 'directory=%s/%s/%s\nfilename=%s/%s/%s\n' '$(@D)' '$(origin @D)' '$(flavor @D)' '$(@F)' '$(origin @F)' '$(flavor @F)'
