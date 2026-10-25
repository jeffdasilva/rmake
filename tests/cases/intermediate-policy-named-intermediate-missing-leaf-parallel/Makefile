seed := $(shell touch -d @1600000002 out)
.INTERMEDIATE: mid
all: out
out: mid
	@echo out=$@; cp -p $< $@
mid: input
	@echo mid=$@; cp -p $< $@
