$(file >seen,anchor)
unexport HOME READY RESULT
HOME = $(file >>seen,HOME)$(shell printf hdir)
RESULT := missing
define ACTION
MAKEFLAGS += -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)] STATUS=[$(.SHELLSTATUS)])
endef
.PHONY: all
all:;@$(eval $(value ACTION))true
