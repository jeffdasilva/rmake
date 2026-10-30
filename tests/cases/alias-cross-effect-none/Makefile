R0=$(R1)
R1=$(file >>seen,E)$(eval LEAF:=changed)$(LEAF)
LEAF=before
$(info VALUE=[$(R0)])
$(info EFFECT=[$(words $(file <seen))])
.PHONY: all
all:;
