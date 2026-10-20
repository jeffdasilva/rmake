seed := $(shell touch -d @1600000000.1 out;touch -d @1600000000.1 input)
.LOW_RESOLUTION_TIME: out
all: out
out: input;@echo "$@:$?";touch -d @1600000002 out
