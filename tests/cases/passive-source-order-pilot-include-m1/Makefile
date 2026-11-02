$(file >seen,anchor)
include step.mk
all:;@$(eval unexport READY STATE A B)$(info RESULT STATE=[$(STATE)] A=[$(A)] B=[$(B)] GNU=[$(GNUMAKEFLAGS)])$(file >>seen,DONE)$(info SEEN=[$(file <seen)]):
