$(info READ:$(MAKE_RESTARTS))
-include inc.mk
all:
	@echo ALL:$(CONTENT)
ifeq ($(MAKE_RESTARTS),)
inc.mk:: force1
	@echo WRITEFAIL; printf "CONTENT := failed\n" > inc.mk; exit 7
inc.mk:: force2
	@echo CREATE2; printf "CONTENT := tail\n" > inc.mk
.PHONY: force1 force2 bad bad2
bad:
	@echo BAD; exit 3
bad2:
	@echo BAD2; exit 4
endif
