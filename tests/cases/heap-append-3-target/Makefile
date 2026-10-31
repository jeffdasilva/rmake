R0 = $(if yes,[$(R1)])
R1 = $(if yes,[$(R2)])
R2 = $(if yes,[$(R3)])
R3=base
.PHONY: all
all: R3+=$(if yes,local)
all:;@$(info VALUE=[$(R0)]):
