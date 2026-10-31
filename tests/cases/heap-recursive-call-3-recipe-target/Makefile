walk=$(if $(1),x$(call walk,$(wordlist 2,9999,$(1))))
1=global
.PHONY: all
all:;@$(info VALUE=[$(call walk,word word word)])$(info AFTER=[$(1)]):
