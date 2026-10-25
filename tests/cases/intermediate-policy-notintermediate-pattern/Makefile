seed := $(shell touch -d @1600000000 x.src)
.NOTINTERMEDIATE: %.mid
all: x.out
%.out: %.mid
	@cp -p $< $@
%.mid: %.src
	@cp -p $< $@
