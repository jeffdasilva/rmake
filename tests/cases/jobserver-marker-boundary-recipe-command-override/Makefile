.PHONY: all
all: export MAKEFLAGS := x --jobserver-auth=opaque -- X=a b
all:
	@python3 -c 'import os; print(os.environ["MAKEFLAGS"])'
