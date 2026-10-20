all: a b c
a b &:: input;@echo first $@:$^;touch -d @1000000000 a b
b c &:: other;@echo second $@:$^;touch -d @1000000000 b c
b:: extra;@echo plain $@:$^
input other extra:;@echo $@;touch $@
