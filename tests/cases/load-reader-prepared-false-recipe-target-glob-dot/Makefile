HOME = $(file >>seen,HOME)~
HOME = $(file >>seen,HOME)modules
$(file >>seen,before)
.PHONY: all
all:
	@$(eval load $(file >>seen,operand)./plug*())$(file >>seen,recipe)printf "DONE\n"
