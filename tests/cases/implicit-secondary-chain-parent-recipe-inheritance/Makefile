.SECONDEXPANSION:
all: V=parent
all: one.out
%.out: $$(info OUT=$$(V))%.mid
	@echo out=$(V); touch $@
%.mid: $$(info MID=$$(V))%.in
	@echo mid=$(V); touch $@
