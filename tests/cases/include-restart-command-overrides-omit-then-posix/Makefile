PHASE := $(or $(MAKE_RESTARTS),initial)
$(file >>seen,READ|$(PHASE)|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(value MAKEFLAGS)|$(value -*-command-variables-*-))

ifeq ($(MAKE_RESTARTS),)
MAKEOVERRIDES =
override undefine MARK
MAKEFLAGS += --silent
MAKEOVERRIDES = $(file >>seen,PUBLIC|$(PHASE)|$(origin MARK))MARK=from-public SIDE=$(origin MARK)
else
.POSIX:
MAKEOVERRIDES = $(file >>seen,PUBLIC|$(PHASE)|$(origin MARK))MARK=from-public SIDE=$(origin MARK)
override undefine MARK
MAKEFLAGS += --silent
endif

include generated.mk

all:
	@$(file >>seen,GOAL|$(PHASE)|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(value MAKEFLAGS)|$(value -*-command-variables-*-)|$(MAKEFLAGS))\
	printf "GOAL_ENV|%s\n" "$$MAKEFLAGS" >>seen; test "$(READY)" = yes && cat seen

generated.mk:
	@$(file >>seen,REMAKE|$(PHASE)|$(origin MARK)|$(value MARK)|$(origin SIDE)|$(value SIDE)|$(value MAKEFLAGS)|$(value -*-command-variables-*-))\
	printf "REMAKE_ENV|%s\n" "$$MAKEFLAGS" >>seen; printf "READY := yes\n" >$@
