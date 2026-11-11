$(file >seen,initial)
D := unrelated
NAME := D(inner)
define $(NAME)
scope=$(1)
endef
RESULT := $(call $(NAME),leaf)
$(info result=[$(RESULT)])
$(file >>seen,result=[$(RESULT)])
.PHONY: check
check: ; @:
