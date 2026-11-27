PHASE := read
$(file >>seen,ENTRY|$(origin MARK)|$(value MARK)|$(value MAKEFLAGS)|$(value EARLY))
override undefine MARK
MAKEFLAGS += --silent
.POSIX:
$(file >>seen,READ|$(origin MARK)|$(value MARK)|$(value MAKEFLAGS)|$(value EARLY))
PHASE := run
all:
	@$(file >>seen,RUN|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(value MAKEFLAGS))printf 'ENV|%s\n' "$$MAKEFLAGS" >>seen; cat seen
