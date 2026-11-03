$(file >seen,anchor)
unexport HOME
HOME = $(file >>seen,HOME)same
MAKEFLAGS += -I~/inc
MAKEFLAGS += -Isame//inc -I~/inc
$(info DIRS=[$(filter same%,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
