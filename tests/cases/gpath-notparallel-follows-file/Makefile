seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1600000002 input)
VPATH=src
GPATH=src
.NOTPARALLEL: foo.out
all: foo.out
foo.out: first second
	@echo "@=[$@] *=[$*] <=[$<] ^=[$^] +=[$+] |=[$|] %=[$%] V=[$(V)]"
first: ; @echo first
second: ; @echo second
