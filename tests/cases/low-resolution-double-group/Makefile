seed := $(shell touch -d @1600000000.1 a b c;touch -d @1600000000.2 input)
.LOW_RESOLUTION_TIME: a b c
all: a b c
a b &:: input;@echo first;touch -d @1600000002 a b
b c &:: input;@echo second;touch -d @1600000002 b c
