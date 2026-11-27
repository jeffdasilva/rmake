PHASE := read
$(file >seen,ENTRY|$(origin MARK)|$(value MARK)|$(value MAKEFLAGS))
.POSIX:
MAKEOVERRIDES = $(file >>seen,ALIAS|$(PHASE)|$(origin MARK))MARK=from-public SIDE=$(origin MARK)
override undefine MARK
override MAKEFLAGS := --silent -- MARK=price$$(-*-command-variables-*-) SIDE=published
$(file >>seen,READ|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(flavor MAKEFLAGS)|$(value MAKEFLAGS))
PHASE := run
all:
	@$(file >>seen,RUN|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(flavor MAKEFLAGS)|$(value MAKEFLAGS))$(file >>seen,CONSUME-BEGIN)$(file >>seen,CONSUME|$(MARK))printf 'ENV|%s|\n' "$$MAKEFLAGS" >>seen; cat seen
