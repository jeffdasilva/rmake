export LEX := global
define DECL
owned: CAP := $(eval unexport LEX)$(shell printf "[%s]" "$$LEX")
endef
$(foreach LEX,outer,$(info before=$(shell printf "[%s]" "$$LEX"))$(let LEX,inner,$(eval $(value DECL))$(info inner=$(shell printf "[%s]" "$$LEX")))$(info restored=$(shell printf "[%s]" "$$LEX")))
owned:;@$(info cap=$(CAP) after=$(shell printf "[%s]" "$$LEX")):
