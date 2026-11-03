HOME = $(file >>seen,HOME)~
HOME = $(file >>seen,HOME)$(error HOME-stop)
$(file >>seen,before)
$(eval load $(file >>seen,operand)bad() ~/later)
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
