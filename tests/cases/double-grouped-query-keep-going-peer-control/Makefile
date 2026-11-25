.SECONDEXPANSION:
a: deps = c
b: deps = $(file >>witness,expanded) d e
a b &:: $$(deps) ; @:
b:: standalone ; +@touch unexpected-branch
c: ; @:
d: ; +@printf "forced\n" >> witness
e: ; +@touch peer-tail
standalone: ;
