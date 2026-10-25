VPATH = src
all: foo
	@echo all=$^ first=$<
src/foo: input
	@echo actual=$@; cp -p $< $@
