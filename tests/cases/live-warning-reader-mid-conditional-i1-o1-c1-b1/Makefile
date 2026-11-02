GNUMAKEFLAGS:=
A=base
B=base
$(file >seen,anchor)
override MAKEFLAGS = A?=$$(file\ >>seen,A)$$(MISSÿ_A) --warn-undefined-variables B?=$$(file\ >>seen,B)$$(MISSÿ_B)
$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)])
all:;@:
