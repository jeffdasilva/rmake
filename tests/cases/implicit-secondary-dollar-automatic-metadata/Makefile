.SECONDEXPANSION:
all: one.out
%.out: $$(info O=$$(origin *) F=$$(flavor *) RAW=$$(value *))%.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
