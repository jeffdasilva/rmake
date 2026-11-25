.SECONDEXPANSION:
a: deps = c
b: deps = $(file >>unexpected,expanded) d
a b &:: $$(deps) ; @:
c: ; +@exit 1
d: ; +@touch unexpected
