VPATH = src
all: foo
	@echo all=$^ first=$<
foo: VALUE=source
src/%: VALUE+=pattern
src/foo: ; @echo actual=$@:$(VALUE)
