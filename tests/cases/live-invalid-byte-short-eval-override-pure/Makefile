A=before
GNUMAKEFLAGS :=
$(file >seen,anchor)
$(eval override MAKEFLAGS = -ÿs)
$(file >>seen,END:$(A))$(info MFLAGS=[$(MFLAGS)] A=[$(A)] O=[$(origin A)] F=[$(flavor A)])$(info SEEN=[$(file <seen)])
all:;@:
