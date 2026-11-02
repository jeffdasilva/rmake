A=before
GNUMAKEFLAGS :=
$(file >seen,anchor)
MAKEFLAGS = --w -s A:=after
$(file >>seen,END:$(A))$(info MFLAGS=[$(MFLAGS)] A=[$(A)] O=[$(origin A)] F=[$(flavor A)])$(info SEEN=[$(file <seen)])
all:;@:
