HOME = $(file >>seen,HOME)~
$(file >>seen,before)
include child.mk
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
