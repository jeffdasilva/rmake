R0 = $(let 1,shadow,[$(R1)])
R1 = $(let 1,shadow,[$(R2)])
R2 = $(let 1,shadow,[$(R3)])
R3=$(call ID)
ID=$(1)
F=$(R0)
1=global
.PHONY: all
all:;@$(info MASK=[$(call F,outer)])$(info LEX=[$(let 1,lex,$(call F))])$(info AFTER=[$(1)]):
