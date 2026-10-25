seed := $(shell touch -d @1600000000 src/foo.out input src/bar.out; touch -d @1600000002 input)
VPATH=src
GPATH=src
all: foo.out bar.out
foo.out bar.out &: input
	@echo owner=$@ all=$^; cp input src/foo.out; cp input src/bar.out
