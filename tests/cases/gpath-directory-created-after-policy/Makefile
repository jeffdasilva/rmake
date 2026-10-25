seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1600000002 input)
VPATH=late
GPATH=late
all: prepare foo.out
prepare: ; @mkdir late; echo old >late/foo.out; touch -d @1600000000 late/foo.out
foo.out: input
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] V=[$(V)]"
