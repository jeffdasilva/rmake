$(info READ:$(MAKE_RESTARTS))
all:
	@echo ALL:$(CONTENT)
Makefile:: absent
	@echo BRANCH:1
Makefile:: force2
	@echo BRANCH:2
.PHONY: bad force1 force2 force3
bad:
	@echo BAD; exit 5
