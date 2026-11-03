HOME = $(file >>seen,HOME)~
$(file >>seen,before)
.PHONY: all
all:
	@$(eval -load $(file >>seen,operand)$(error load-stop)$(file >>seen,forbidden))$(file >>seen,recipe)printf "DONE\n"
