.SECONDEXPANSION:
all: one.out create two.out
%.out: $$(info TRY=$$*)missing
	@echo build=$@
create: ; @touch missing; exit 7
