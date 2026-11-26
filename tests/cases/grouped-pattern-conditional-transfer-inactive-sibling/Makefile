caller: X := owned
override undefine X
X := root
%.out: X := pattern
one.out peer.out &: early caller late
	@$(info parent=[$(X)|$(origin X)]):
early: ; @$(info early=[$(X)|$(origin X)]):
caller: ; @$(eval peer.out: X ?= $$(error unexpected conditional suffix)):
late: ; @$(info late=[$(X)|$(origin X)]):
