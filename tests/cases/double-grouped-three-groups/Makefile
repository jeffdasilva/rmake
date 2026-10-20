all: a b c d
a b &::;@echo first $@;touch -d @1000000000 a b
b c &::;@echo second $@;touch -d @1000000000 b c
a d &::;@echo third $@;touch -d @1000000000 a d
