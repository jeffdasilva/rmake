PHASE := read
$(file >seen,ENTRY|$(origin MARK)|$(value MARK)|$(value MAKEFLAGS))
MAKEOVERRIDES = $(file >>seen,ALIAS|$(PHASE)|$(origin MARK))MARK=from-public SIDE=$(origin MARK)
override undefine MARK
override MAKEFLAGS = --silent -- MARK=price-tag SIDE=published
$(file >>seen,READ|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(flavor MAKEFLAGS)|$(value MAKEFLAGS))
PHASE := run
all:
	@$(file >>seen,RUN|$(origin MARK)|$(value MARK)|$(MARK)|$(origin SIDE)|$(value SIDE)|$(flavor MAKEFLAGS)|$(value MAKEFLAGS))printf 'ENV|%s\n' "$$MAKEFLAGS" >>seen; cat seen
