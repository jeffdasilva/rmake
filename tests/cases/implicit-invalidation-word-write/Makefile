.SECONDEXPANSION:
all: one.out two.out
%.out: $$(if $$(filter two,$$*),$$(file >missing,created),)$$(info TRY=$$*)missing
	@echo build=$@
