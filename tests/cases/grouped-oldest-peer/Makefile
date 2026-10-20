seed := $(shell touch -d @1000000000 a;touch -d @1000000010 input;touch -d @1000000020 b)
all: b a
a b &: input;@echo "$@:$?";touch a b
