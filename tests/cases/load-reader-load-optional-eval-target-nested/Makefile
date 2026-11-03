HOME = $(file >>seen,HOME)~
$(file >>seen,before)
$(eval -load $(eval SIDE := $(file >>seen,operand))$(SIDE))
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
