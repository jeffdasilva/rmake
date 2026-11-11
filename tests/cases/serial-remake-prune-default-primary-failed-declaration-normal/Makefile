$(info READ:$(MAKE_RESTARTS))
all:
	@echo ALL:$(CONTENT)
Makefile:: force1
	@echo BRANCH:1; exit 7
Makefile:: force2
	@echo BRANCH:2
.PHONY: bad force1 force2 force3
bad:
	@echo BAD; exit 5
