.DEFAULT_GOAL := all
LEX := global
define INNER
inner: SNAP := $(shell printf "[%s]" "$$LEX")
endef
define OUTER
owner: CAP := $(eval export LEX)$(eval $(value INNER))$(shell printf "[%s]" "$$LEX")
endef
$(foreach LEX,local,$(eval $(value OUTER))$(info outer=$(shell printf "[%s]" "$$LEX")))
all: owner inner
owner:;@$(info owner=$(CAP) after=$(shell printf "[%s]" "$$LEX")):
inner:;@$(info inner=$(SNAP)):
