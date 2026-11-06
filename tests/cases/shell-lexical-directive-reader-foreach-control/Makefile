LEX := global
$(foreach LEX,local,$(eval export LEX)$(info reader=$(shell printf "[%s]" "$$LEX")))
export LEX
all:;@$(info recipe-expansion=$(shell printf "[%s]" "$$LEX"))printf "recipe=[%s]\n" "$$LEX"
