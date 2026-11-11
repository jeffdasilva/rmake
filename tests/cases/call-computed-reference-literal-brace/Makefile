$(file >seen,initial)
D := unrelated
NAME := D}tail
define $(NAME)
scope=$(1)
endef
RESULT := $(call $(NAME),leaf)
$(info result=[$(RESULT)])
$(file >>seen,result=[$(RESULT)])
.PHONY: check
check: ; @:
