R0 = $(R1)
R1 = $(R2)
R2 = $(R3)
R3 = $(1)
F = $(R0)
$(info CALL=[$(call F,outer)])
$(info MASK=[$(let 1,lex,$(call F))])
$(info LET=[$(let 1,lex,$(R0))])
$(info LOOP=[$(foreach 1,a b,$(R0))])
.PHONY: all
all:;
