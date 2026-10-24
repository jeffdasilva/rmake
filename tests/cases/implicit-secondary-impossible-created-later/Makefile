.SECONDEXPANSION:
all: one.out two.out
%.out: $$(info TRY=$$*)missing
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
two.out: $$(shell touch missing)
