R0=base
R1=<$(VALUE)>
VALUE=global
.PHONY: all
all: VALUE=local
all: R0+=[$(R1)]
all:;@$(info VALUE=[$(R0)]):
