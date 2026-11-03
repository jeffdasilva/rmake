HOME = $(file >>seen,HOME)~
PAYLOAD = $(file >>seen,operand)
$(file >>seen,before)
include child.mk
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
