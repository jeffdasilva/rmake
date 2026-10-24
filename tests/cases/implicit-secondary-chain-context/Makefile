.SECONDEXPANSION:
all: V=parent
all: one.out
%.out: $$(info OUT=$$(V))%.mid
	@echo out; touch $@
%.mid: $$(info MID=$$(V))%.in
	@echo mid; touch $@
