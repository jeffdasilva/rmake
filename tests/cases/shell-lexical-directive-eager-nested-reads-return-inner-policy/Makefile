.DEFAULT_GOAL := all
export LEX := global
define INNER
inner: SNAP := $(eval unexport LEX)$(shell printf "[%s]" "$$LEX")
endef
define OUTER
owner: CAP := $(eval $(value INNER))$(shell printf "[%s]" "$$LEX")
endef
$(foreach LEX,local,$(eval $(value OUTER))$(info outer=$(shell printf "[%s]" "$$LEX")))
all: owner inner
owner:;@$(info owner=$(CAP) after=$(shell printf "[%s]" "$$LEX")):
inner:;@$(info inner=$(SNAP)):
