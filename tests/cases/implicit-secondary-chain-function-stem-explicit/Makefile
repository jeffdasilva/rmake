.SECONDEXPANSION:
all: one.out
%.out: $$(addsuffix .mid,$$*)
	@echo out; touch $@
%.mid: $$(info MID=$$*)%.in
	@echo mid; touch $@
