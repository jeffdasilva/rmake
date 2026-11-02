$(file >seen,anchor)
override GNUMAKEFLAGS = A=$$(file\ >>seen,A)$$(MISSING_Aÿ) B=$$(file\ >>seen,B)$$(MISSING_Bÿ) --warn-undefined-variables -e
all:;@$(eval unexport A B)$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)]):
