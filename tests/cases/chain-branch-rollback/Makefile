all: x.out
%.out: %.mid missing;@echo wrong
%.out: %.src;@echo right
%.mid: %.src;@echo wrong;touch $@
