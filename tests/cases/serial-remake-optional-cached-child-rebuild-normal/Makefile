$(info READ:$(MAKE_RESTARTS))
-include inc.mk
all:
	@echo ALL:$(CONTENT)
inc.mk:: bad
	@echo BRANCH:1
inc.mk:: force2
	@echo BRANCH:2; printf "CONTENT := created\n" > inc.mk
.PHONY: bad force1 force2 force3 force4
bad:
	@echo BAD; exit 5
