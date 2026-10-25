seed := $(shell touch -d @1600000000 src/foo; touch -d @1600000002 input)
VPATH = src
all: foo
	@echo all=$^ first=$<
foo: VALUE=source
foo: input
	@echo original=$@:$(VALUE); cp -p input $@
src/foo: VALUE=destination
