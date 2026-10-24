.SECONDEXPANSION:
all: one.out
one.mid: V=explicit
%.out: $$(info OUT=$$(V))%.mid
	@echo out; touch $@
%.mid: $$(info MID=$$(V))%.in
	@echo mid=$(V); touch $@
