$(file >seen,anchor)
unexport HOME X
HOME = hdir
MAKEFLAGS = X:=$$(file\ >>seen,X)$$(eval\ MAKEFLAGS\ =\ --include-dir=~/inc\ --file)x V1:=$$(file\ >>seen,V1)v1
$(info X=[$(X)] V1=[$(V1)] DIRS=[$(filter hdir/inc,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
