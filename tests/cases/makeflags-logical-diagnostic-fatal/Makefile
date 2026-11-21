a.out: MAKEFLAGS += -k
a.out:
	@$(eval MAKEFLAGS = $$(if $$(ON),$$(error outer),-s))$(eval ON=1)$(info value=[$(MAKEFLAGS)]):
