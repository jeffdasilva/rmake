LEX := global
all:;@$(foreach LEX,local,$(eval owned: CAP := $$(eval export LEX)$$(shell printf "[%s]" "$$$$LEX"))$(info outer=$(shell printf "[%s]" "$$LEX"))):
owned:;@$(info cap=$(CAP) after=$(shell printf "[%s]" "$$LEX")):
