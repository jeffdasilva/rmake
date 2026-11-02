A=before
GNUMAKEFLAGS :=
$(file >seen,anchor)
all:;@$(eval MAKEFLAGS = -s --not-an-option -k)$(file >>seen,END:$(A))$(info MFLAGS=[$(MFLAGS)] A=[$(A)] O=[$(origin A)] F=[$(flavor A)])$(info SEEN=[$(file <seen)]):
