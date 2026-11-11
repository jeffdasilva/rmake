$(info READ:$(MAKE_RESTARTS))
-include inc.mk
all:
	@echo ALL:$(CONTENT)
inc.mk:: force1
	@echo BRANCH:1; exit 7
inc.mk:: force2
	@echo BRANCH:2
inc.mk:: force3
	@echo BRANCH:3
.PHONY: bad force1 force2 force3 force4
bad:
	@echo BAD; exit 5
