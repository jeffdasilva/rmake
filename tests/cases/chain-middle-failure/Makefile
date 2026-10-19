all: x.out
%.out: %.mid;@echo final:$<:$*;cp $< $@
%.mid: %.src;@touch $@;exit 7
