$(file >seen,initial)
OTHER := D$$1
define $(OTHER)
escaped
endef
NAME := D$$$$1
define $(NAME)
stub
endef
RESULT := $(call $(NAME),leaf)
$(info result=[$(RESULT)])
$(file >>seen,result=[$(RESULT)])
.PHONY: check
check: ; @:
