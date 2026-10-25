VPATH = src
all: foo
	@echo all=$^ first=$<
foo: ; @echo original=$@
src/foo: input
	@echo actual=$@; cp -p $< $@
.PHONY: foo
