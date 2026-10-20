all: a b c
a b &:: source;@echo first $@;touch -d @1600000000.000000004 a b
b c &:: source;@echo second $@;touch -d @1600000000.000000004 b c
