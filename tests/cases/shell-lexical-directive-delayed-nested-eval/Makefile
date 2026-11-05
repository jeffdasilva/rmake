export LEX := global
all:;@$(foreach LEX,local,$(eval $$(eval unexport LEX))$(info inside=$(shell printf "[%s]" "$$LEX")))$(info after=$(shell printf "[%s]" "$$LEX")):
