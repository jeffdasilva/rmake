.SECONDEXPANSION:
all: one.out
one.out: extra
%.out: $$(info newer=[$$?])%.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] ?=[$?] V=[$(V)]"
extra: ;
