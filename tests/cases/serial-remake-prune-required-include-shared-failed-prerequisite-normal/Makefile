$(info READ:$(MAKE_RESTARTS))
include inc.mk
all:
	@echo ALL:$(CONTENT)
inc.mk:: bad
	@echo BRANCH:1
inc.mk:: bad
	@echo BRANCH:2
.PHONY: bad force1 force2 force3
bad:
	@echo BAD; exit 5
