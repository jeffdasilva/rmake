seed := $(shell touch -d @1600000000.1 out;touch -d @1600000000.2 input)
.SECONDEXPANSION:
.LOW_RESOLUTION_TIME: out
all: out
out: $$(warning second)input;@echo rebuild
