seed := $(shell touch -d @1600000000 src/foo.out input policy.in; touch -d @1600000002 input policy.in 2>/dev/null)
VPATH=src
include policy.mk
all: foo.out
foo.out: input
	@echo target=$@ restarts=$(MAKE_RESTARTS); cp -p $< $@
policy.mk: policy.in
	@cp -p $< $@
