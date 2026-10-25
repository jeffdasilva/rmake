VPATH = src
all: item.out
%.out: %.in
	@echo implicit=$@:$<; cp -p $< $@
src/item.in: input
	@echo actual=$@; cp -p $< $@
