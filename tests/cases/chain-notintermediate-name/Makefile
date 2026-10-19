all: a.out b.out
%.out: %.mid;@cp $< $@
%.mid: %.src;@cp $< $@
.NOTINTERMEDIATE: a.mid
