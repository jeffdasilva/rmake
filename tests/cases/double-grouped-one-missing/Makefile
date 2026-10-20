seed := $(shell touch -d @1000000000 input;touch -d @1000000010 a b)
all: a b c
a b &:: input;@echo first;touch -d @1000000000 a b
b c &:: input;@echo second;touch -d @1000000000 b c
