$(file >seen,anchor)
override GNUMAKEFLAGS = $(file >>seen,SOURCE)$(eval MAKEFLAGS += --warn-undefined-variables) A:=$$(file\ >>seen,A)$$(MISSING_A) B:=$$(file\ >>seen,B)$$(MISSING_B) --warn-undefined-variables
all:;@$(eval unexport A B)$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)]):
