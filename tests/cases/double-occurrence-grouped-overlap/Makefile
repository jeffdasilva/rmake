all: a b c
a b a &:: input;@echo first $@:$+;touch -d @1000000000 a b
b c b &:: other;@echo second $@:$+;touch -d @1000000000 b c
input other:;@touch $@
