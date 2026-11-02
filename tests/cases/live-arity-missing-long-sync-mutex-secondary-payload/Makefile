A=before
GNUMAKEFLAGS :=
$(file >seen,anchor)
define DO
$(eval MAKEFLAGS = A:=after --sync-mutex)$(file >>seen,END:$(A))$(info MFLAGS=[$(MFLAGS)] A=[$(A)] O=[$(origin A)] F=[$(flavor A)] CC=[$(origin CC)])$(info SEEN=[$(file <seen)])
endef
.SECONDEXPANSION:
all: $$(DO)
	@:
