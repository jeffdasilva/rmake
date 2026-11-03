$(file >seen,anchor)
unexport HOME X
HOME = hdir
MAKEFLAGS = X:=$$(file\ >>seen,X)$$(eval\ MAKEFLAGS\ =\ --include-dir=~/inc\ --)x V1:=$$(file\ >>seen,V1)v1 V2:=$$(file\ >>seen,V2)v2 V3:=$$(file\ >>seen,V3)v3
$(info X=[$(X)] V1=[$(V1)] V2=[$(V2)] V3=[$(V3)] DIRS=[$(filter hdir/inc,$(.INCLUDE_DIRS))])
.PHONY: all
all:;
