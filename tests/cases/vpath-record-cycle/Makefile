VPATH = src
all: foo
	@echo all=$^ first=$<
foo: src/foo
src/foo: foo
	@echo actual=$@
