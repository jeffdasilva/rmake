HOME = $(file >>seen,HOME)~
HOME = $(file >>seen,HOME)modules
$(file >>seen,before)
$(eval load $(file >>seen,operand)plug*())
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
