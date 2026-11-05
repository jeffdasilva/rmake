all: private export LEX := target
all:;@$(info before=$(shell printf "[%s]" "$$LEX"))$(foreach LEX,local,$(info lexical=$(LEX) inside=$(shell printf "[%s]" "$$LEX")))$(info after=$(shell printf "[%s]" "$$LEX")):
