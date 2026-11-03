HOME = $(file >>seen,HOME)~
$(file >>seen,before)
$(eval -load $(file >>seen,operand)./plugin() $(file >>seen,last))
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
