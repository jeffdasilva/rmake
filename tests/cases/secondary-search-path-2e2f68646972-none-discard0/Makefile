$(file >seen,anchor)
unexport HOME READY RESULT A B
READY :=
RESULT := missing
HOME = $(file >>seen,HOME)$(if $(READY),./hdir,$(eval READY:=yes)~)
MAKEFLAGS += -Ikeep -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter keep ./hdir/inc,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
