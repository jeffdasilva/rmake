$(file >seen,initial)
Dleaf := must-not-expand
NAME := D$$1
define $(NAME)

endef
RESULT := $(call $(NAME),$(file >>seen,arg)leaf)
$(info result=[$(RESULT)])
$(file >>seen,result=[$(RESULT)])
.PHONY: check
check: ; @:
