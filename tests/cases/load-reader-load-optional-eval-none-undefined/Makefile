HOME = $(file >>seen,HOME)~
$(file >>seen,before)
$(eval -load $(UNSET))
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
