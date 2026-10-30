R0=base
R1=$(LEAF)
LEAF=global
.PHONY: all
all: R0 += $(R1)
all: LEAF=local
all:;@$(info VALUE=[$(R0)]):
