$(info READ:$(MAKE_RESTARTS))
-include inc.mk
all: inc.mk
	@echo ALL:$(CONTENT)
ifeq ($(MAKE_RESTARTS),)
inc.mk:: force2
	@echo NOTHING
inc.mk:: bad
	@echo ROOT:bad
.PHONY: force1 force2 bad bad2
bad:
	@echo BAD; exit 3
bad2:
	@echo BAD2; exit 4
endif
