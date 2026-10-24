.SECONDEXPANSION:
all: one.out
%.mid: V := base
%.mid: V += $(info pattern-activation)mid
%.out: $$(info OUT=$$(V))%.mid
	@echo out=$(V); touch $@
%.mid: $$(info MID=$$(V))%.in
	@echo mid=$(V); touch $@
