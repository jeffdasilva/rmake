$(file >seen,anchor)
unexport HOME RESULT
HOME = $(error HOME must remain lazy)
RESULT := missing
define ACTION
MAKEFLAGS += -I~rmake-no-such-user-b9fe8a/inc
-include answer.mk
$(info RESULT=[$(RESULT)])
endef
.PHONY: all
all:;@$(eval $(value ACTION))true
