# Copyright 2026 Jeff Da Silva
# SPDX-License-Identifier: GPL-3.0-or-later

CAPTURE_0 := $(origin @):$(flavor @):[$(value @)]
CAPTURE_1 := $(origin %):$(flavor %):[$(value %)]
CAPTURE_2 := $(origin <):$(flavor <):[$(value <)]
CAPTURE_3 := $(origin ?):$(flavor ?):[$(value ?)]
CAPTURE_4 := $(origin ^):$(flavor ^):[$(value ^)]
CAPTURE_5 := $(origin +):$(flavor +):[$(value +)]
CAPTURE_6 := $(origin *):$(flavor *):[$(value *)]
CAPTURE_7 := $(origin |):$(flavor |):[$(value |)]
CAPTURE_8 := $(origin @D):$(flavor @D):[$(value @D)]
CAPTURE_9 := $(origin @F):$(flavor @F):[$(value @F)]
CAPTURE_10 := $(origin %D):$(flavor %D):[$(value %D)]
CAPTURE_11 := $(origin %F):$(flavor %F):[$(value %F)]
CAPTURE_12 := $(origin <D):$(flavor <D):[$(value <D)]
CAPTURE_13 := $(origin <F):$(flavor <F):[$(value <F)]
CAPTURE_14 := $(origin ?D):$(flavor ?D):[$(value ?D)]
CAPTURE_15 := $(origin ?F):$(flavor ?F):[$(value ?F)]
CAPTURE_16 := $(origin ^D):$(flavor ^D):[$(value ^D)]
CAPTURE_17 := $(origin ^F):$(flavor ^F):[$(value ^F)]
CAPTURE_18 := $(origin +D):$(flavor +D):[$(value +D)]
CAPTURE_19 := $(origin +F):$(flavor +F):[$(value +F)]
CAPTURE_20 := $(origin *D):$(flavor *D):[$(value *D)]
CAPTURE_21 := $(origin *F):$(flavor *F):[$(value *F)]
CAPTURE_22 := $(origin |D):$(flavor |D):[$(value |D)]
CAPTURE_23 := $(origin |F):$(flavor |F):[$(value |F)]
.PHONY: all
all:
	@printf 'parse %s:%s\n' '@' '$(CAPTURE_0)'
	@printf 'parse %s:%s\n' '%' '$(CAPTURE_1)'
	@printf 'parse %s:%s\n' '<' '$(CAPTURE_2)'
	@printf 'parse %s:%s\n' '?' '$(CAPTURE_3)'
	@printf 'parse %s:%s\n' '^' '$(CAPTURE_4)'
	@printf 'parse %s:%s\n' '+' '$(CAPTURE_5)'
	@printf 'parse %s:%s\n' '*' '$(CAPTURE_6)'
	@printf 'parse %s:%s\n' '|' '$(CAPTURE_7)'
	@printf 'parse %s:%s\n' '@D' '$(CAPTURE_8)'
	@printf 'parse %s:%s\n' '@F' '$(CAPTURE_9)'
	@printf 'parse %s:%s\n' '%D' '$(CAPTURE_10)'
	@printf 'parse %s:%s\n' '%F' '$(CAPTURE_11)'
	@printf 'parse %s:%s\n' '<D' '$(CAPTURE_12)'
	@printf 'parse %s:%s\n' '<F' '$(CAPTURE_13)'
	@printf 'parse %s:%s\n' '?D' '$(CAPTURE_14)'
	@printf 'parse %s:%s\n' '?F' '$(CAPTURE_15)'
	@printf 'parse %s:%s\n' '^D' '$(CAPTURE_16)'
	@printf 'parse %s:%s\n' '^F' '$(CAPTURE_17)'
	@printf 'parse %s:%s\n' '+D' '$(CAPTURE_18)'
	@printf 'parse %s:%s\n' '+F' '$(CAPTURE_19)'
	@printf 'parse %s:%s\n' '*D' '$(CAPTURE_20)'
	@printf 'parse %s:%s\n' '*F' '$(CAPTURE_21)'
	@printf 'parse %s:%s\n' '|D' '$(CAPTURE_22)'
	@printf 'parse %s:%s\n' '|F' '$(CAPTURE_23)'
