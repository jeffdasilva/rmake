.SECONDEXPANSION:
all: a b c
a b &:: $$(info expand $$@)first;@echo first $@;touch -d @1000000000 a b
b c &:: $$(info expand $$@)second;@echo second $@;touch -d @1000000000 b c
first second:;@echo $@;touch $@
