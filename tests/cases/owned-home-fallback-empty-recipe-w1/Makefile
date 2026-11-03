$(file >seen,anchor)
unexport HOME RESULT
HOME = $(file >>seen,HOME)
RESULT := missing
define ACTION
MAKEFLAGS += -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)])
endef
.PHONY: all
all:;@$(eval $(value ACTION))true
