p := $(shell touch -d @1000000000 a b;touch -d @1000000010 extra)
all: a b
a b &:;@echo "$@:$^:$?";touch a b
b: extra
