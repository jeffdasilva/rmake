caller: X := owned
override undefine X
X := root
%.out: X := pattern
one.out peer.out &: ; @$(info built=[$(X)|$(origin X)]):
caller: ; @$(eval one.out: X ?= $$(error unexpected conditional suffix))$(eval one.out: CHECK := $$(info retained=[$$(X)|$$(origin X)])):
