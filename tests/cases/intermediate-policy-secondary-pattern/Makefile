seed := $(shell touch -d @1600000000 x.src)
.SECONDARY: %.mid
all: x.out
%.out: %.mid
	@cp -p $< $@
%.mid: %.src
	@cp -p $< $@
