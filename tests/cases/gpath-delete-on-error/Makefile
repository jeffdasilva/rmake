seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1600000002 input)
VPATH=src
GPATH=src
.DELETE_ON_ERROR:
all: foo.out
foo.out: input
	@echo changed > $@; exit 7
