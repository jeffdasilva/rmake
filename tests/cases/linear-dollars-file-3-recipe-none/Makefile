R0 = pre$$<$(R1)>$$post$
R1 = pre$$<$(R2)>$$post$
R2 = pre$$<$(R3)>$$post$
R3 = $(file >>seen,E)end
NAME:=LEAF
LEAF:=end
ID=$(1)
.PHONY: all
all:;@$(info VALUE=[$(R0)])$(info EFFECT=[$(words $(file <seen))]):
