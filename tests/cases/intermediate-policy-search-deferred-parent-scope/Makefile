seed := $(shell touch -d @1600000002 out; touch -d @1600000000 input)
.SECONDARY:
.SECONDEXPANSION:
all: out
out: VALUE=inherited
out: x.mid
	@echo out; cp -p $< $@
x.mid: $$(info value=$$(VALUE))input
	@echo mid; cp -p $< $@
