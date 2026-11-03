HOME = $(file >>seen,HOME)~
$(file >>seen,before)
$(eval load $(file >>seen,operand)$(error load-stop)$(file >>seen,forbidden))
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
