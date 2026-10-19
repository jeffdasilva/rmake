.ONESHELL:
all: left right;@test -f left && test -f right
left right:
	@value=$@
	printf %s "$$value" > $@
