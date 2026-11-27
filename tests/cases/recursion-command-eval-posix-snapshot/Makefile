PHASE := read
$(file >>seen,ENTRY|$(origin MARK)|$(value MARK)|$(value MAKEFLAGS)|$(value EARLY))
.POSIX:
MAKEOVERRIDES = $(file >>seen,ALIAS|$(PHASE)|$(origin MARK))MARK=from-public SIDE=$(origin MARK)
override undefine MARK
MAKEFLAGS += --silent
$(file >>seen,READ|$(origin MARK)|$(value MARK)|$(value MAKEFLAGS)|$(value EARLY))
PHASE := run
all:
	@$(file >>seen,RUN|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(origin NOT_AN_OVERRIDE)|$(value MAKEFLAGS))printf 'ENV|%s\n' "$$MAKEFLAGS" >>seen; exec "$(MAKE)" --file=child.mk child
