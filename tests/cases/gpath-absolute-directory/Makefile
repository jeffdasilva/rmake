seed := $(shell touch -d @1600000000 src/foo.out input; touch -d @1600000002 input)
VPATH=$(CURDIR)/src
GPATH=$(CURDIR)/src
all: foo.out
foo.out: input
	@echo $(patsubst $(CURDIR)/%,%,$@)
