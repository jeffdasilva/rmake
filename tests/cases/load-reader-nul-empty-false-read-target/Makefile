HOME = $(file >>seen,HOME)~
$(file >>seen,before)
load $(file >>seen,operand)$(file <payload)
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
