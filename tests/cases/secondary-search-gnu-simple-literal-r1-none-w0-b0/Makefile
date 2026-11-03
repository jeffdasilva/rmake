$(file >seen,anchor)
unexport HOME READY RESULT
READY :=
RESULT := missing
HOME := $(file >>seen,HOME)~
GNUMAKEFLAGS += -I~/inc
.PHONY: all
all:;@$(eval -include answer.mk)$(info RESULT=[$(RESULT)] DIRS=[$(filter ~/inc hdir/inc,$(.INCLUDE_DIRS))] RAW=[$(filter -I~/inc,$(MAKEFLAGS))])true
