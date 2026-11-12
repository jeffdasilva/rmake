$(file >seen,initial)
BASE := AA$$0
define $(BASE)
selected-zero
endef
NAME := A$$0
define $(NAME)
stub
endef
RESULT := $(call $(NAME),leaf)
$(info result=[$(RESULT)])
$(file >>seen,result=[$(RESULT)])
.PHONY: check
check: ; @:
