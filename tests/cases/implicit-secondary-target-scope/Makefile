.SECONDEXPANSION:
all: one.out
one.out: V=local
%.out: $$(info V=$$(V))$$(V).in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
