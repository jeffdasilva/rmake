$(file >seen,anchor)
unexport HOME READY RESULT A B
READY :=
RESULT := missing
HOME = $(file >>seen,HOME)~
MAKEFLAGS += -I~/inc -Obad A:=$$(file\ >>seen,A)done
$(file >>seen,FORBIDDEN)
.PHONY: all
all:;
