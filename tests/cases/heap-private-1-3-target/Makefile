R0 = $(if yes,[$(R1)])
R1 = $(if yes,[$(R2)])
R2 = $(if yes,[$(R3)])
R3=$(VALUE)
VALUE=global
.PHONY: all child
all: private VALUE=parent
all: child;@$(info P=[$(R0)]):
child:;@$(info C=[$(R0)]):
