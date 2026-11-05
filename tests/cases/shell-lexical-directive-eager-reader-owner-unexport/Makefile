export LEX := global
define DECL
owned: CAP := $(eval unexport LEX)$(shell printf "[%s]" "$$LEX")
endef
$(foreach LEX,local,$(info before=$(shell printf "[%s]" "$$LEX"))$(eval $(value DECL))$(info outer=$(shell printf "[%s]" "$$LEX")))
owned:;@$(info cap=$(CAP) after=$(shell printf "[%s]" "$$LEX")):
