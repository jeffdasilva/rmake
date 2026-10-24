seed := $(shell touch -d @1600000000.1 one.out;touch -d @1600000000.2 one.in)
.LOW_RESOLUTION_TIME: one.out
.SECONDEXPANSION:
all: one.out
%.out: $$(warning candidate)%.in
	@echo recipe
