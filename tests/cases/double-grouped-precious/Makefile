.DELETE_ON_ERROR:
.PRECIOUS: b
all: a b c
a b &::;@touch -d @1000000000 a b;exit 7
b c &::;@echo second $@;touch -d @1000000000 b c
