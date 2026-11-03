$(file >seen,anchor)
unexport HOME READY RESULT
HOME = $(file >>seen,HOME)$(shell printf hdir)
RESULT := missing
MAKEFLAGS += -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)] STATUS=[$(.SHELLSTATUS)])
.PHONY: all
all:;
