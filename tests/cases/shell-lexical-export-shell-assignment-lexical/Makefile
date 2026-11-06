export LEX := global
all:;@$(foreach LEX,scoped,$(eval CAP != printf "[%s]" "$$$$LEX")$(info local=$(LEX) cap=$(CAP)))$(info after=$(shell printf "[%s]" "$$LEX")):
