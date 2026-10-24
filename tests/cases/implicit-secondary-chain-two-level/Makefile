.SECONDEXPANSION:
all: one.out
%.out: $$(info OUT=$$*)%.mid
	@echo out; touch $@
%.mid: $$(info MID=$$*)%.stage
	@echo mid; touch $@
%.stage: $$(info STAGE=$$*)%.in
	@echo stage; touch $@
