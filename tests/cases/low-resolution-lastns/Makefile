seed := $(shell touch -d @1600000000 out;touch -d @1600000000.999999999 input)
.LOW_RESOLUTION_TIME: out
all: out
out: input;@echo "$@:$?";touch -d @1600000002 out
