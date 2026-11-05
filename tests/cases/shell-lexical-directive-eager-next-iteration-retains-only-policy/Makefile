LEX := global
define DECL
owner: CAP := $(eval export LEX)$(shell printf "[%s]" "$$LEX")
endef
$(foreach LEX,one two,$(if $(filter one,$(LEX)),$(eval $(value DECL)))$(info outer=$(shell printf "[%s]" "$$LEX")))
owner:;@$(info cap=$(CAP) after=$(shell printf "[%s]" "$$LEX")):
