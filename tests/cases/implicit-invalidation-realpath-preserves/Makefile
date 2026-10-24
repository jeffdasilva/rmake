.SECONDEXPANSION:
all: one.out create two.out
%.out: $$(info TRY=$$*)missing
	@echo build=$@
create: ; @$(if $(realpath control),,)
