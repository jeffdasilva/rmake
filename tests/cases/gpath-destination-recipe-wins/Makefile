seed := $(shell touch -d @1600000000 src/foo.out input extra; touch -d @1600000002 input extra)
VPATH=src
GPATH=src
all: foo.out
foo.out: input
	@echo original=$@
src/foo.out: extra
	@echo resolved=$@ first=$< all=$^
