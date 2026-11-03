HOME = $(file >>seen,HOME)~
$(file >>seen,before)
load $(if ,$(error forbidden),$(file >>seen,operand))
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
