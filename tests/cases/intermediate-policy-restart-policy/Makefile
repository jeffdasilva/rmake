seed := $(shell touch -d @1600000002 out; touch -d @1600000000 input; touch -d @1600000000 policy.in)
include policy.mk
all: out
out: mid
	@echo out=$@; cp -p $< $@
mid: input
	@echo mid=$@; cp -p $< $@
policy.mk: policy.in
	@cp -p $< $@
