HOME = $(file >>seen,HOME)~
$(file >>seen,before)
.PHONY: all
all:
	@$(eval -load $(eval SIDE := $(file >>seen,operand))$(SIDE))$(file >>seen,recipe)printf "DONE\n"
