LEX := global
owner: LEX := target
define DECL
owner: CAP := $(eval unexport LEX)$(shell printf "[%s]" "$$LEX")
endef
$(foreach LEX,local,$(eval export LEX)$(eval $(value DECL))$(info outer=$(shell printf "[%s]" "$$LEX")))
owner:;@$(info cap=$(CAP) after=$(shell printf "[%s]" "$$LEX")):
