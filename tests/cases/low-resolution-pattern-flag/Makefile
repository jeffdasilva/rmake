seed := $(shell touch -d @1600000000 x.out;touch -d @1600000000.1 x.in)
.LOW_RESOLUTION_TIME: %.out
all: x.out
%.out: %.in;@echo $@;touch x.out
