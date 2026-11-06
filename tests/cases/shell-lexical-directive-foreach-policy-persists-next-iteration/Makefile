LEX := global
all:;@$(foreach LEX,one two,$(if $(filter one,$(LEX)),$(eval export LEX))$(info inside=$(shell printf "[%s]" "$$LEX")))$(info after=$(shell printf "[%s]" "$$LEX")):
