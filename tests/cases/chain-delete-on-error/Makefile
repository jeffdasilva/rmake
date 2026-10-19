.DELETE_ON_ERROR:
all: x.out
%.out: %.mid;@echo final:$<:$*;touch $@;exit 7
%.mid: %.src;@echo middle:$<:$*;cp $< $@
