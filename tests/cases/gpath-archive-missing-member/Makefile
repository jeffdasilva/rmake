seed := $(shell touch -d @1600000000 src/foo.out input src/lib.a; touch -d @1600000002 input)
VPATH=src
GPATH=src
all: lib.a(two.o)
lib.a(two.o): input
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] V=[$(V)]"
