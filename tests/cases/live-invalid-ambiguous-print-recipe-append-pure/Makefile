A=before
GNUMAKEFLAGS :=
$(file >seen,anchor)
all:;@$(eval MAKEFLAGS += --pr)$(file >>seen,END:$(A))$(info MFLAGS=[$(MFLAGS)] A=[$(A)] O=[$(origin A)] F=[$(flavor A)])$(info SEEN=[$(file <seen)]):
