A=before
GNUMAKEFLAGS :=
$(file >seen,anchor)
all:;@$(eval MAKEFLAGS = --jobserver-fds)$(file >>seen,END:$(A))$(info MFLAGS=[$(MFLAGS)] A=[$(A)] O=[$(origin A)] F=[$(flavor A)] CC=[$(origin CC)])$(info SEEN=[$(file <seen)]):
