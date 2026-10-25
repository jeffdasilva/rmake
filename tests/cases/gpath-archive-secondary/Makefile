seed := $(shell touch -d @1600000000 src/foo.out input src/lib.a; touch -d @1600000002 input)
.SECONDEXPANSION:
VPATH=src
GPATH=src
all: lib.a(one.o)
lib.a(one.o): $$(info AT=$$@ MEMBER=$$%)input
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] V=[$(V)]"
