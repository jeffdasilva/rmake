.SECONDEXPANSION:
all: one.out
%.out: $$(info OUT=$$*)$$*.mid
	@echo out; touch $@
%.mid: $$(info MID=$$*)%.in
	@echo mid; touch $@
