$(file >seen,anchor)
$(eval override GNUMAKEFLAGS = READY:=yes STATE:=$$$$(if\ $$$$(value\ GNUMAKEFLAGS),pending,cleared) A:=$$$$(file\ >>seen,A)$$$$(info\ GNU_A))
override MAKEFLAGS = $(if $(filter environment,$(origin READY)), $(file >>seen,FLAGS)$(info FLAGS_STATE=[$(STATE)] A=[$(A)]) B:=two)
all:;@$(eval unexport READY STATE A B)$(info RESULT STATE=[$(STATE)] A=[$(A)] B=[$(B)] GNU=[$(GNUMAKEFLAGS)])$(file >>seen,DONE)$(info SEEN=[$(file <seen)]):
