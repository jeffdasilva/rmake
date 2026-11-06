.PHONY: all
all: export MAKEFLAGS := x --jobserver-auth=opaque -- X=a b
all:
	$(info $(shell python3 -c 'import os; print(os.environ["MAKEFLAGS"])'))@:
