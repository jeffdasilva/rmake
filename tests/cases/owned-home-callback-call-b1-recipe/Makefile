$(file >seen,anchor)
unexport HOME READY RESULT
homedir = hdirÿ
HOME = $(file >>seen,HOME)$(call homedir)
RESULT := missing
define ACTION
MAKEFLAGS += -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)])
endef
.PHONY: all
all:;@$(eval $(value ACTION))true
