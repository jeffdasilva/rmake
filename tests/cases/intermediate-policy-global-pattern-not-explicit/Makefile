seed := $(shell touch -d @1600000002 x.out; touch -d @1600000000 x.src)
.SECONDARY:
.NOTINTERMEDIATE: %.mid
.INTERMEDIATE: x.mid
all: x.out
%.out: %.mid
	@cp -p $< $@
%.mid: %.src
	@cp -p $< $@
