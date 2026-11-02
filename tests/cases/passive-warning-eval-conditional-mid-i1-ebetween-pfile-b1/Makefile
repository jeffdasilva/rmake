$(file >seen,anchor)
A=baseA
B=baseB
$(eval override GNUMAKEFLAGS = A?=$$$$(file\ >>seen,A)$$$$(MISSING_Aÿ) -e --warn-undefined-variables B?=$$$$(file\ >>seen,B)$$$$(MISSING_Bÿ))
all:;@$(eval unexport A B)$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)]):
