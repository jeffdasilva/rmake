export LEX := global
all:;@$(let LEX,outer,$(info outer-before=$(shell printf "[%s]" "$$LEX"))$(let OTHER,inner,$(eval unexport LEX)$(info inner=$(shell printf "[%s]" "$$LEX")))$(info outer-after=$(shell printf "[%s]" "$$LEX")))$(info after=$(shell printf "[%s]" "$$LEX")):
