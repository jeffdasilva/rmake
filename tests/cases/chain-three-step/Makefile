all: x.out
%.out: %.mid;@echo final:$<:$*;cp $< $@
%.mid: %.raw;@echo middle:$<:$*;cp $< $@
%.raw: %.src;@cp $< $@
