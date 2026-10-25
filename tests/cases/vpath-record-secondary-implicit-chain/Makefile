.SECONDEXPANSION:
VPATH=src
all: item.out
%.out: $$*.mid
	@echo final=$@:$<
%.mid: $$*.in
	@echo middle=$@:$<; cp -p $< $@
src/item.in: input
	@echo leaf=$@; cp -p $< $@
