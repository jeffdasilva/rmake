caller: X := owned
override undefine X
X := root
%.out: X := pattern
one.out peer.out &: early caller late
	@$(info parent=[$(X)|$(origin X)]):
early: ; @$(info early=[$(X)|$(origin X)]):
caller: ; @$(let X,lexical,$(eval one.out: X ?= $$(error unexpected conditional suffix))$(info lexical=[$(X)])):
late: ; @$(info late=[$(X)|$(origin X)]):
