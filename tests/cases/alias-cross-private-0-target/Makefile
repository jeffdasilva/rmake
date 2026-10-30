R0=$(R1)
R1=$(VALUE)
VALUE=global
.PHONY: all parent child
all: parent
parent: VALUE=parent
parent: child
parent:;@$(info P=[$(R0)]):
child:;@$(info C=[$(R0)]):
