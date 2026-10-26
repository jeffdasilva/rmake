.DEFAULT_GOAL := all
all: lib.a(alpha-long-member.o)
lib.a(alpha-long-member.o):;@:
.PHONY: verify
VERIFY ?= gnu
verify:;@python3 verify.py expected-$(VERIFY).a
