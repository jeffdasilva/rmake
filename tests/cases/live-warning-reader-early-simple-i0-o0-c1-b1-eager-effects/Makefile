GNUMAKEFLAGS:=
A=base
B=base
$(file >seen,anchor)
MAKEFLAGS = --warn-undefined-variables A:=$$(file\ >>seen,A)$$(MISSÿ_A) B:=$$(file\ >>seen,B)$$(MISSÿ_B)
$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)])
all:;@:
