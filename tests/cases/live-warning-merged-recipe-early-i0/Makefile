GNUMAKEFLAGS:=
A=base
B=base
$(file >seen,anchor)
all:;@$(eval MAKEFLAGS = --warn-undefined-variables A:=$$$$(info\ AT_A)$$$$(file\ >>seen,A)$$$$(MISSING_A) B:=$$$$(info\ AT_B)$$$$(file\ >>seen,B)$$$$(MISSING_B))$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)]):
