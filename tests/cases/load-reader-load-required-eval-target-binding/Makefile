HOME = $(file >>seen,HOME)~
PAYLOAD = $(file >>seen,operand)
$(file >>seen,before)
$(eval load $(PAYLOAD))
$(file >>seen,after)
.PHONY: all
all:
	@$(file >>seen,recipe)printf "DONE\n"
