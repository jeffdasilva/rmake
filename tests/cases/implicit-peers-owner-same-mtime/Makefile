seed := $(shell touch -d @1000000000 x.a;touch -d @1000000010 input)
all: x.a x.b
%.a %.b: input;@echo $@;touch -d @1000000000 $@
