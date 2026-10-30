R0=[$(R1)]
R1=<$(VALUE)>
VALUE=global
.PHONY: all child
all: VALUE=parent
all: child;@$(info P=[$(R0)]):
child:;@$(info C=[$(R0)]):
