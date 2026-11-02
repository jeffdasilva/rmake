R0=$(file >>seen,b0)$(eval $$(R1))$(file >>seen,a0)
R1=$(file >>seen,b1)$(eval $$(R2))$(file >>seen,a1)
R2=$(file >>seen,b2)$(eval $$(R3))$(file >>seen,a2)
R3=$(info LEAF)
$(R0)
$(info AFTER)
$(info EVENTS=[$(file <seen)])
.PHONY: all
all:;
