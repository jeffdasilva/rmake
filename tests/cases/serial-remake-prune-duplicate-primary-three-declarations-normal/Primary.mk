ifeq ($(primary_read),)
primary_read := 1
$(info READ:$(MAKE_RESTARTS))
all:
	@echo ALL:$(CONTENT)
Primary.mk:: absent
	@echo BRANCH:1
Primary.mk:: force2
	@echo BRANCH:2
Primary.mk:: force3
	@echo BRANCH:3; exit 7
.PHONY: bad force1 force2 force3
bad:
	@echo BAD; exit 5
endif
