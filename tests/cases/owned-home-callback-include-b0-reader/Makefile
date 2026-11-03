$(file >seen,anchor)
unexport HOME READY RESULT
HOME = $(file >>seen,HOME)$(eval include setter.mk)$(READY)
RESULT := missing
MAKEFLAGS += -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)] READY=[$(READY)])
.PHONY: all
all:;
