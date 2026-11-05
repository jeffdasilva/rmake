export LEX := global
all: private export LEX := target
all:;@$(info before=$(shell printf "[%s]" "$$LEX"))$(foreach LOOP,local,$(info lexical=$(LOOP) inside=$(shell printf "[%s]" "$$LEX")))$(info after=$(shell printf "[%s]" "$$LEX")):
