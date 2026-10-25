seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1600000002 input)
VPATH=src
GPATH=src
foo.out: input
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] V=[$(V)]"
middle: ; @echo middle
