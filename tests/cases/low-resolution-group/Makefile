seed := $(shell touch -d @1600000000 a b;touch -d @1600000000.1 input)
.LOW_RESOLUTION_TIME: a b
all: a b
a b &: input;@echo $@;touch a b
