R0 = $(if yes,[$(R1)])
R1 = $(if yes,[$(R2)])
R2 = $(if yes,[$(R3)])
R3=$(file <payload)$(file >>seen,E)
.PHONY: all
all:;@$(info VALUE=[$(R0)])$(info EFFECT=[$(words $(file <seen))]):
