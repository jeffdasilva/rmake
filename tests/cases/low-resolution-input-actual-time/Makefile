seed := $(shell touch -d @1600000000.9 input;touch -d @1600000000.1 out)
.LOW_RESOLUTION_TIME: input
all: out
out: input;@echo "$@:$?";touch -d @1600000002 out
