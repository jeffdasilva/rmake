seed := $(shell touch -d @1000000000 x.b;touch -d @1000000010 input;touch -d @1000000020 x.a)
all: x.a x.b
%.a %.b: input;@echo "$@:$?";touch x.a x.b
