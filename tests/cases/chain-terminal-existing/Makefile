all: x.out
%.out:: %.mid;@echo final:$<:$*;cp $< $@
%.mid: %.src;@echo middle:$<:$*;cp $< $@
