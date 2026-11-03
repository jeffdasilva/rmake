HOME = $(file >>seen,HOME)~
$(file >>seen,before)
-load # $(error ignored)
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
