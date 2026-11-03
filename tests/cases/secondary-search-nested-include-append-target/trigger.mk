MAKEFLAGS += -I~/inc A:=$$(file\ >>seen,A)one B:=$$(file\ >>seen,B)two
-include answer.mk
$(info RESULT=[$(RESULT)] DIRS=[$(filter hdir/inc extra,$(.INCLUDE_DIRS))] A=[$(A)] B=[$(B)])
