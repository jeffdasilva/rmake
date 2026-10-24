.SECONDEXPANSION:
all: one.out
%.out: $$(info A)$$(strip missing $$(info B)other) $$(info NEVER)
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
