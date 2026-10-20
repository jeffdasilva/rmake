all: a b c
a: X=A
b: X=B
c: X=C
a b &::;@echo first $@ $(X);touch -d @1000000000 a b
b c &::;@echo second $@ $(X);touch -d @1000000000 b c
