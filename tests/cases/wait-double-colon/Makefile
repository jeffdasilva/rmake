all:: a .WAIT b;@echo first
all:: c .WAIT d;@echo second
a c:;@touch $@
b:;@test -f a;touch b
d:;@test -f c;touch d
