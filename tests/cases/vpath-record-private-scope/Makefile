VPATH=src
all: foo
foo: private VALUE=hidden
foo: input
src/foo: child
	@echo actual=$@:$(VALUE)
child: ; @echo child=$(VALUE)
