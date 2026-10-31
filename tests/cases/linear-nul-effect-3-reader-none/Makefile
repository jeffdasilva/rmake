R0 = [$(R1)]
R1 = [$(R2)]
R2 = [$(R3)]
R3 = $(file <payload)$(file >>seen,E)
$(info VALUE=[$(R0)])$(info EFFECT=[$(words $(file <seen))])
.PHONY: all
all:;
