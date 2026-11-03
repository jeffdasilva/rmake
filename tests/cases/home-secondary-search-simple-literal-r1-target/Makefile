$(file >seen,anchor)
unexport HOME READY RESULT
READY :=
HOME := $(file >>seen,HOME)~
MAKEFLAGS += -I~/inc
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter ~/inc hdir/inc,$(.INCLUDE_DIRS))] RAW=[$(filter -I~/inc,$(MAKEFLAGS))])
.PHONY: all
all:;
