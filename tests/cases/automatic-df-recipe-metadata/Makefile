# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

.PHONY: all dir/out a p/b p/c order
all: dir/out
dir/out: a p/b a p/c | order p/b
	@$(foreach n,@ % < ? ^ + * | @D @F %D %F <D <F ?D ?F ^D ^F +D +F *D *F |D |F,printf 'recipe %s:%s:%s:[%s]:[%s]\n' '$(n)' '$(origin $(n))' '$(flavor $(n))' '$(value $(n))' '$($(n))';)
a p/b p/c order:
	@:
