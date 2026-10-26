$(shell python3 setup.py)
.DEFAULT_GOAL := all
all: lib.a(one.o)
lib.a(one.o):
	@python3 update.py
	@echo archive=$(file <phase)
.PHONY: verify
verify:;@python3 verify.py
