VPATH = src
all: sub/foo
	@echo all=$^
src/sub/foo: input
	@echo actual=$@; cp -p input $@
