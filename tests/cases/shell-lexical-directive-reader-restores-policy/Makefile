LEX := global
$(foreach LEX,local,$(eval export LEX)$(info reader=$(shell printf "[%s]" "$$LEX")))
all:;@$(info after=$(shell printf "[%s]" "$$LEX"))printf "recipe=[%s]\n" "$$LEX"
