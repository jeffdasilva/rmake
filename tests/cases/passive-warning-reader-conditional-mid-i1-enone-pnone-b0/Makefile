$(file >seen,anchor)
override GNUMAKEFLAGS = A?=$$(file\ >>seen,A)$$(MISSING_A) --warn-undefined-variables B?=$$(file\ >>seen,B)$$(MISSING_B)
all:;@$(eval unexport A B)$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)]):
