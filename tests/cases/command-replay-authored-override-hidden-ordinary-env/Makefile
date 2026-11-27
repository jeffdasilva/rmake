PHASE := read
$(file >>seen,ENTRY|$(origin MARK)|$(value MARK)|$(origin MAKEFLAGS)|$(value MAKEFLAGS))
MAKEOVERRIDES = $(file >>seen,ALIAS|$(PHASE)|$(origin MARK))MARK=from-public SIDE=$(origin MARK)
override undefine MARK
override MAKEFLAGS = --silent -- $(-*-command-variables-*-)
$(file >>seen,READ|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(origin MAKEFLAGS)|$(flavor MAKEFLAGS)|$(value MAKEFLAGS))
PHASE := run
all:
	@$(file >>seen,RUN|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(origin MAKEFLAGS)|$(value MAKEFLAGS))printf "ENV|%s\n" "$$MAKEFLAGS" >>seen; cat seen
