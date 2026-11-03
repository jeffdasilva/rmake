$(file >seen,anchor)
HOME := $(file >>seen,HOME)hdir
ANSWER := missing
MAKEFLAGS += -I~/inc
-include answer.mk
$(info ANSWER=[$(ANSWER)] DIRS=[$(filter hdir/inc,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
