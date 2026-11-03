$(file >seen,anchor)
unexport HOME READY RESULT
HOME = $(file >>seen,HOME)$(eval READY := yes)hdir
RESULT := missing
define ACTION
MAKEFLAGS += -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)] READY=[$(READY)])
endef
.PHONY: all
all:;@$(eval $(value ACTION))true
