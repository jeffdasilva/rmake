VPATH=src
all: lib.a(m.o)
src/lib.a: original.a
	@cp -p $< $@
src/lib.a(m.o): src/lib.a
	@echo member=$@:$%
