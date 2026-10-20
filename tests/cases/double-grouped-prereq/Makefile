all: a b c
a b &:: input;@echo first $@:$^;touch -d @1000000000 a b
b c &:: more;@echo second $@:$^;touch -d @1000000000 b c
input more:;@echo $@;touch $@
