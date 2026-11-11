$(info READ:$(MAKE_RESTARTS))
all:
	@echo ALL:$(CONTENT)
Primary.mk:: bad
	@echo BRANCH:1
Primary.mk:: force2
	@echo BRANCH:2
.PHONY: bad force1 force2 force3
bad:
	@echo BAD; exit 5
