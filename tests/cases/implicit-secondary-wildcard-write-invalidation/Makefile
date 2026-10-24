.SECONDEXPANSION:
all: one.out
%.out: $$(shell touch one1.in)$$(wildcard one*.in)
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
