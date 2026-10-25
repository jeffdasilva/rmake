seed := $(shell touch -d @1600000000 src/foo; touch -d @1600000002 input)
VPATH = src
all: foo
	@echo all=$^ first=$<
foo: ; @echo original=$@
src/foo: input
	@echo actual=$@; cp -p $< $@
