HOME = $(file >>seen,HOME)~
PAYLOAD = $(file >>seen,operand)
$(file >>seen,before)
.PHONY: all
all:
	@$(eval -load $(PAYLOAD))$(file >>seen,recipe)printf "DONE\n"
