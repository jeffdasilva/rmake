.SECONDEXPANSION:
all: one.out two.out
%.out: $$(info first=$$*)missing
	@echo first=$@
%.out: $$(if $$(filter two,$$*),$$(file >missing,created),)$$(info second=$$*)othermissing
	@echo second=$@
