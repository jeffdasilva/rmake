.PHONY: all a b
all: a b
a:
	@python3 helper.py a
b:
	@python3 helper.py b
	@python3 helper.py first
	@python3 helper.py second
