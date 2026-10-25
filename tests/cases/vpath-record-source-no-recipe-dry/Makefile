VPATH = src
all: foo
	@echo all=$^ first=$<
foo: input
src/foo: ; @echo actual=$@ prereqs=$^
