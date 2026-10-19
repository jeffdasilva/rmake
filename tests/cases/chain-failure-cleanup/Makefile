all: x.out
%.out: %.mid;@echo final:$<:$*;exit 7
%.mid: %.src;@echo middle:$<:$*;cp $< $@
