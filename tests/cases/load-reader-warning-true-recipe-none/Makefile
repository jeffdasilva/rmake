HOME = $(file >>seen,HOME)~
$(file >>seen,before)
.PHONY: all
all:
	@$(eval -load $(file >>seen,operand)$(UNSET))$(file >>seen,recipe)printf "DONE\n"
