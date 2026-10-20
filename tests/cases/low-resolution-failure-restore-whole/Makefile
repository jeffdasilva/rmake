seed := $(shell touch -d @1600000000 out;touch -d @1600000001 input)
.DELETE_ON_ERROR:
.LOW_RESOLUTION_TIME: out
out: input;@touch -d @1600000000 out;exit 7
