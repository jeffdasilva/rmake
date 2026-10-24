seed := $(shell touch -d @1500000000 one.in;touch -d @1600000000 one.out)
.SECONDEXPANSION:
all: one.out
%.out: $$(shell rm -f one.out)%.in
	@echo recipe
