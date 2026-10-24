.SECONDEXPANSION:
all: one.out
%.out: V=pattern
%.out: $$(info V=$$(V))$$(V).in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
