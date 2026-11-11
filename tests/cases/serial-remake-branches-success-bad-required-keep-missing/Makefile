$(info READ:$(MAKE_RESTARTS))
include inc.mk
all:
	@echo ALL:$(CONTENT)
ifeq ($(MAKE_RESTARTS),)
inc.mk:: force1
	@echo CREATE1; printf "CONTENT := head\n" > inc.mk
inc.mk:: bad
	@echo ROOT:bad
.PHONY: force1 force2 bad bad2
bad:
	@echo BAD; exit 3
bad2:
	@echo BAD2; exit 4
endif
