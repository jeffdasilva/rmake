.DEFAULT_GOAL := all
$(file >>seen,read-$(MAKE_RESTARTS))

ifeq ($(MAKE_RESTARTS),)
Makefile: trigger
endif

.PHONY: trigger
trigger:

Makefile:
	@printf 'changed\n' >> changed

all:
	@$(file >>seen,goal)printf 'ordinary\n'
