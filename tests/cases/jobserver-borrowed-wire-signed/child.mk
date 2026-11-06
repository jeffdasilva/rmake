$(info read=[$(filter --jobserver-auth=%,$(MAKEFLAGS))])
.PHONY: all recursive ordinary
all: recursive ordinary
recursive:
	+@python3 witness.py recursive
ordinary:
	@python3 witness.py ordinary
