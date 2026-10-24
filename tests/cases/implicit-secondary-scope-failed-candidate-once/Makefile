.SECONDEXPANSION:
all: one.out
%.out: V := base
%.out: V += $(info activation)append
%.out: $$(info first=$$(V))absent
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
%.out: $$(info second=$$(V))%.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
