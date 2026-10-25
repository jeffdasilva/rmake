VPATH=src
all: item.out
%.out: lib.a(m.o)
	@echo implicit=$@:$<
src/lib.a(m.o): ; @echo member=$@:$%
