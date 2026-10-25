seed := $(shell touch -d @1600000000 src/foo.out input src/lib.a; touch -d @1600000002 input)
VPATH=src
GPATH=src
.PHONY: lib.a
all: lib.a(one.o)
lib.a(one.o): input
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] V=[$(V)]"
