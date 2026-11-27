PHASE := read
$(file >seen,ENTRY|$(origin MARK)|$(value MARK)|$(value MAKEFLAGS))
MAKEOVERRIDES :=
override undefine MARK
SWITCH = $(file >>seen,PREFIX|$(PHASE))--silent
override MAKEFLAGS = $(SWITCH) -- $(MAKEOVERRIDES)
$(file >>seen,READ|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(flavor MAKEFLAGS)|$(value MAKEFLAGS))
PHASE := run
all:
	@$(file >>seen,RUN|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(flavor MAKEFLAGS)|$(value MAKEFLAGS))$(file >>seen,CONSUME-BEGIN)$(file >>seen,CONSUME|$(MARK))printf 'ENV|%s|\n' "$$MAKEFLAGS" >>seen; cat seen
