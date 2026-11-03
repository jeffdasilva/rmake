HOME = $(file >>seen,HOME)~
HOME = $(file >>seen,HOME)$(error HOME-stop)
$(file >>seen,before)
include child.mk
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
