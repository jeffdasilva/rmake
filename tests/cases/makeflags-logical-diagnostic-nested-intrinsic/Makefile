FAIL = $(intcmp bad,0)
a.out: MAKEFLAGS += -k
a.out:
	@$(eval MAKEFLAGS = $$(if $$(ON),$$(FAIL),-s))$(eval ON=1)$(info value=[$(MAKEFLAGS)]):
