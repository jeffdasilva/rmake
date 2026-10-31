R0 = x$(R1)
R1 = x$(R2)
R2 = x$(R3)
R3 = $(eval LEAF:=changed)$(LEAF)
NAME:=LEAF
LEAF:=end
ID=$(1)
.PHONY: all
all:;@$(info VALUE=[$(R0)]):
