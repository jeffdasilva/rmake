seed := $(shell touch -d @1600000000 input)
.NOTINTERMEDIATE:
.INTERMEDIATE: mid
all: out
out: mid
	@echo out=$@; cp -p $< $@
mid: input
	@echo mid=$@; cp -p $< $@
