override undefine MARK
MAKEOVERRIDES =
MAKEFLAGS += --silent
MAKEOVERRIDES = $(file >>seen,OVERRIDES|$(MAKELEVEL)|$(origin MARK))MARK=from-public SIDE=$(origin MARK)
MAKEFLAGS += --keep-going
$(file >>seen,READ|$(MAKELEVEL)|$(origin MARK)|$(value MAKEFLAGS))

all:
	@$(file >>seen,PARENT|$(MAKELEVEL)|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE))\
	printf 'INHERITED|%s\n' "$$MAKEFLAGS" >>seen; exec "$(MAKE)" --file=child.mk child
