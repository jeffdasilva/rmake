GNUMAKEFLAGS:=
A=base
B=base
$(file >seen,anchor)
$(eval MAKEFLAGS = A?=$$$$(file\ >>seen,A)$$$$(MISSING_PRE)$$$$(eval\ MAKEFLAGS\ =\ --warn-undefined-variables)$$$$(MISSING_POST) B:=$$$$(file\ >>seen,B)$$$$(MISSING_B) --warn-undefined-variables)
$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)])
all:;@:
