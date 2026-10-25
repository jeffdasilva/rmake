VPATH = src
all: item.out
%.out: %.early
	@echo early=$@:$<
%.out: %.late
	@echo late=$@:$<
src/item.early: ; @echo leaf=$@
