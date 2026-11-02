GNUMAKEFLAGS:=
A=base
B=base
$(file >seen,anchor)
$(eval override MAKEFLAGS = A:=$$$$(file\ >>seen,A)$$$$(MISSING_A) B:=$$$$(file\ >>seen,B)$$$$(MISSING_B) --warn-undefined-variables)
$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)])
all:;@:
