A=before
GNUMAKEFLAGS :=
$(file >seen,anchor)
MAKEFLAGS = --no-keep-going=literal
$(file >>seen,END:$(A))$(info MFLAGS=[$(MFLAGS)] A=[$(A)] O=[$(origin A)] F=[$(flavor A)] CC=[$(origin CC)])$(info SEEN=[$(file <seen)])
all:;@:
