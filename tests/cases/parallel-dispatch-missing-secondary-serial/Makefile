.SECONDEXPANSION:
all: one two
one: ; @./missing-program
two: $$(info sibling-secondary)
	@$(info forbidden-recipe-expansion)printf two > two
