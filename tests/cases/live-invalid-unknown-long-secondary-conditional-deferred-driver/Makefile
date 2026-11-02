A=before
GNUMAKEFLAGS :=
$(file >seen,anchor)
DO = $(eval MAKEFLAGS ?= --not-an-option A:=after)$(file >>seen,END:$(A))$(info MFLAGS=[$(MFLAGS)] A=[$(A)] O=[$(origin A)] F=[$(flavor A)])$(info SEEN=[$(file <seen)])
.SECONDEXPANSION:
all: $$(DO)
	@:
