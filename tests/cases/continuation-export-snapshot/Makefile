export VALUE=$(shell cat value)
all: target
target:
	@printf "%s\n" "$$VALUE" > first;echo changed > value
	@printf "%s\n" "$$VALUE" > second
