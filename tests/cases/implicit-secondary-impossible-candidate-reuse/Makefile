.SECONDEXPANSION:
all: one.out
%.out: $$(info first)missing
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
%.out: $$(info second)missing $$(info later)
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
