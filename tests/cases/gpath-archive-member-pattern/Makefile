seed := $(shell touch -d @1600000000 src/foo.out input src/lib.a one.in; touch -d @1600000002 input one.in)
VPATH=src
GPATH=src
all: lib.a(one.o)
(%.o): %.in
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] V=[$(V)]"
