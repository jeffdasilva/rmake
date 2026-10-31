R0 = [$(R1)]
R1 = [$(R2)]
R2 = [$(R3)]
R3 = $(file >>seen,E)$(filter R0,$(.VARIABLES))
$(info VALUE=[$(R0)])$(info EFFECT=[$(words $(file <seen))])
.PHONY: all
all:;
