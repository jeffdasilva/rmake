.SECONDEXPANSION:
DEPS=a .WAIT b
all: $$(DEPS);@echo "$^:$+"
a:;@touch a
b:;@test -f a;touch b
