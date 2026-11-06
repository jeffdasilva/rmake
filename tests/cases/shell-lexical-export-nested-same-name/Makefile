export LEX := global
all:;@$(foreach LEX,outer,$(info before=$(shell printf "[%s]" "$$LEX"))$(let LEX,inner,$(info inner=$(shell printf "[%s]" "$$LEX")))$(info after-inner=$(shell printf "[%s]" "$$LEX")))$(info after=$(shell printf "[%s]" "$$LEX")):
