export LEX := global
all:;@$(foreach LEX,local,$(info before=$(shell printf "[%s]" "$$LEX"))$(eval unexport LEX)$(info inside=$(shell printf "[%s]" "$$LEX")))$(info after=$(shell printf "[%s]" "$$LEX"))printf "recipe=[%s]\n" "$$LEX"
