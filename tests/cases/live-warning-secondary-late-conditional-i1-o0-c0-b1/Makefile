GNUMAKEFLAGS:=
A=base
B=base
$(file >seen,anchor)
define DO
$(eval MAKEFLAGS = A?=$$$$(file\ >>seen,A)$$$$(MISSÿ_A) B?=$$$$(file\ >>seen,B)$$$$(MISSÿ_B) --warn-undefined-variables)$(info RESULT A=[$(A)]/$(origin A)/$(flavor A) B=[$(B)]/$(origin B)/$(flavor B))$(file >>seen,DONE)$(info SEEN=[$(file <seen)])
endef
.SECONDEXPANSION:
all: $$(DO)
	@:
