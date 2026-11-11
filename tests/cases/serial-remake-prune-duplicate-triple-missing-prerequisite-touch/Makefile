$(info READ:$(MAKE_RESTARTS))
include inc.mk inc.mk
include inc.mk
all:
	@echo ALL:$(CONTENT)
inc.mk:: absent
	@echo BRANCH:1
inc.mk:: force2
	@echo BRANCH:2
.PHONY: bad force1 force2 force3
bad:
	@echo BAD; exit 5
