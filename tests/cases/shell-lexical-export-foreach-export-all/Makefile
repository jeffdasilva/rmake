export
LEX := global
all:;@$(foreach LEX,one two,$(info local=$(LEX) child=$(shell printf "[%s]" "$$LEX")))$(info after=$(shell printf "[%s]" "$$LEX")):
