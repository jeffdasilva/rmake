$(info READ:$(MAKE_RESTARTS))
-include inc.mk
all:
	@echo ALL:$(CONTENT)
ifeq ($(MAKE_RESTARTS),)
inc.mk:: bad
	@echo ROOT:bad
inc.mk:: bad
	@echo ROOT:bad
inc.mk:: force2
	@echo THIRD
.PHONY: force1 force2 bad bad2
bad:
	@echo BAD; exit 3
bad2:
	@echo BAD2; exit 4
endif
