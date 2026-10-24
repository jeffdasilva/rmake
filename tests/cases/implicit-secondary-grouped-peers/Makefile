.SECONDEXPANSION:
all: one.out
%.out %.peer &: $$(info scan=$$*)%.in
	@echo "@=[$@] <=[$<]"; touch $*.out $*.peer
