all:;@$(foreach LEX,local,$(eval export LEX)$(info inside=$(shell printf "[%s]" "$$LEX")))$(info after=$(origin LEX) $(shell printf "[%s]" "$$LEX"))printf "recipe=[%s]\n" "$$LEX"
