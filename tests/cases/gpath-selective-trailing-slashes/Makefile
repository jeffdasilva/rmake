seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1600000002 input)
vpath %.out src//
GPATH=src
all: foo.out
foo.out: input
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] V=[$(V)]"
