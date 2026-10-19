all: x.out
%.out: | %.mid;@echo final:$<:$*;touch $@
%.mid: %.src;@echo middle:$<:$*;cp $< $@
