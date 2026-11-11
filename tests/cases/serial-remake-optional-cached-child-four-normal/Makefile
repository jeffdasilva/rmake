$(info READ:$(MAKE_RESTARTS))
-include inc.mk
all:
	@echo ALL:$(CONTENT)
inc.mk:: bad
	@echo BRANCH:1
inc.mk:: force2
	@echo BRANCH:2
inc.mk:: force3
	@echo BRANCH:3
inc.mk:: force4
	@echo BRANCH:4
.PHONY: bad force1 force2 force3 force4
bad:
	@echo BAD; exit 5
