seed := $(shell touch -d @1000000000 a b)
all: a b
a b &::;@echo group $@;touch -d @1000000000 a b
