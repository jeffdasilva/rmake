.SECONDEXPANSION:
all: one.out
other: missing
%.out: $$(info TRY=$$*)missing
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
%.out: $$(shell touch missing)$$(info fallback)othermissing
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
