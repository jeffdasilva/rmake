.SECONDEXPANSION:
all: one.out
not%.out: $$(error inactive)
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
%.out: %.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
