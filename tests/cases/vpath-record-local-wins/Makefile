seed := $(shell touch -d @1600000000 foo; touch -d @1600000002 input)
VPATH = src
all: foo
	@echo all=$^ first=$<
src/foo: input
	@echo actual=$@; cp -p $< $@
