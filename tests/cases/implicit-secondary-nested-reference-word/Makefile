.SECONDEXPANSION:
all: one.out
V=one
%.out: $$(addsuffix .in,$$(strip $$(V)))
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] V=[$(V)]"
