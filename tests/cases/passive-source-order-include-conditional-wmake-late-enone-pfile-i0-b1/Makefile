$(file >seen,anchor)
A=baseA
B=baseB
include step.mk
all:;@$(eval unexport A B READY STATE)$(info RESULT B=[$(B)]/$(origin B)/$(flavor B) STATE=[$(STATE)] GNU=[$(GNUMAKEFLAGS)])$(file >>seen,DONE)$(info SEEN=[$(file <seen)]):
