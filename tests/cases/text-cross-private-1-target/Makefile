R0=[$(R1)]
R1=<$(VALUE)>
VALUE=global
.PHONY: all child
all: private VALUE=parent
all: child;@$(info P=[$(R0)]):
child:;@$(info C=[$(R0)]):
