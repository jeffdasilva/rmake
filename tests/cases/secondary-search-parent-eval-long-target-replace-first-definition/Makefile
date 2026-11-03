$(file >seen,anchor)
unexport HOME READY RESULT A B C
READY :=
RESULT := missing
B := old
C := old
HOME = $(file >>seen,HOME)$(if $(READY),$(eval HOME:=hdir)$(eval MAKEFLAGS = --silent)hdir,$(eval READY:=yes)~)
define INNER
MAKEFLAGS = -I~/inc
endef
define OUTER
MAKEFLAGS = A:=$$(file\ >>seen,A)$$(eval\ $$(value\ INNER))one B:=$$(file\ >>seen,B)two C:=$$(file\ >>seen,C)three
endef
$(eval $(value OUTER))
-include answer.mk
$(info RESULT=[$(RESULT)] A=[$(A)] B=[$(B)] C=[$(C)])
.PHONY: all
all:;
