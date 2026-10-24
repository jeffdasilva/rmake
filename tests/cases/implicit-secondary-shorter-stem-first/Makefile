.SECONDEXPANSION:
all: aone.out
%.out: $$(info broad=$$*)%.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
a%.out: $$(info narrow=$$*)missing
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
