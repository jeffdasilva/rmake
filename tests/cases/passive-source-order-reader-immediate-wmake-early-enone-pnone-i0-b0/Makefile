$(file >seen,anchor)
override GNUMAKEFLAGS = READY:=yes STATE:=$$(if\ $$(value\ GNUMAKEFLAGS),pending,cleared) A:::=$$(file\ >>seen,A)$$(info\ GNU_A)$$(MISSING_A)
override MAKEFLAGS = $(if $(filter environment,$(origin READY)),$(file >>seen,FLAGS)$(info FLAGS A=[$(A)]/$(origin A)/$(flavor A) STATE=[$(STATE)] GNU=[$(if $(value GNUMAKEFLAGS),pending,cleared)] SEEN=[$(file <seen)]) --warn-undefined-variables B:::=$$(file\ >>seen,B)$$(info\ GNU_B)$$(MISSING_B))
all:;@$(eval unexport A B READY STATE)$(info RESULT B=[$(B)]/$(origin B)/$(flavor B) STATE=[$(STATE)] GNU=[$(GNUMAKEFLAGS)])$(file >>seen,DONE)$(info SEEN=[$(file <seen)]):
