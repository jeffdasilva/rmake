HOME = $(file >>seen,HOME)~
$(file >>seen,before)
.PHONY: all
all:
	@$(eval -load $(if ,$(error forbidden),$(file >>seen,operand)))$(file >>seen,recipe)printf "DONE\n"
