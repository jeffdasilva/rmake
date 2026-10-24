seed := $(shell touch -d @1500000000 one.in)
.SECONDEXPANSION:
all: one.out
%.out: $$(file >one.out,created)%.in
	@echo recipe
