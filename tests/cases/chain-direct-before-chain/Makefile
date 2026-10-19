all: x.out
%.out: %.mid;@echo chained
%.out: %.src;@echo direct
%.mid: %.src;@echo middle;touch $@
