seed := $(shell touch -d @1600000000 out; touch -d @1600000002 input)
.INTERMEDIATE: absent
all: out
out: absent input
	@echo should-not-run
