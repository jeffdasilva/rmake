LEX := global
all:;@$(foreach LEX,local,$(eval export LEX)$(info inside=$(shell printf "[%s]" "$$LEX")))$(info after=$(shell printf "[%s]" "$$LEX"))printf "recipe=[%s]\n" "$$LEX" >>seen; cat seen
