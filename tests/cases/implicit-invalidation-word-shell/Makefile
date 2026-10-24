.SECONDEXPANSION:
all: one.out two.out
%.out: $$(if $$(filter two,$$*),$$(shell touch missing),)$$(info TRY=$$*)missing
	@echo build=$@
