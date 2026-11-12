$(file >seen,initial)
Dleaf := entry.c
NAME := D$$(1):.c=.o
define $(NAME)
stub
endef
RESULT := $(call $(NAME),leaf)
$(info result=[$(RESULT)])
$(file >>seen,result=[$(RESULT)])
.PHONY: check
check: ; @:
