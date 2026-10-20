seed := $(shell touch -d @1600000000 a;touch -d @1600000001 b;touch -d @1600000000.1 input)
.LOW_RESOLUTION_TIME: a
all: a b
a b &: input;@echo $@;touch a b
