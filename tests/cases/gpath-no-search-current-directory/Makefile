seed := $(shell touch -d @1600000000 src/foo.out input foo.out; touch -d @1600000002 input)
VPATH=.
GPATH=.
all: foo.out
foo.out: input
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] V=[$(V)]"
