HOME = $(file >>seen,HOME)~
$(file >>seen,before)
.PHONY: all
all:
	@$(eval -load $(file >>seen,operand)./plugin() $(file >>seen,last))$(file >>seen,recipe)printf "DONE\n"
