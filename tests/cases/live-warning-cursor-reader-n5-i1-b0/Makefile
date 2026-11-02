GNUMAKEFLAGS:=
A=base
B=base
$(file >seen,anchor)
MAKEFLAGS = A:=$$(file\ >>seen,A)$$(eval\ MAKEFLAGS\ =\ --no-print-directory\ --no-print-directory\ --no-print-directory\ --no-print-directory\ --no-print-directory)$$(MISSING_WITHIN) --warn-undefined-variables
$(info RESULT A=[$(A)] FLAGS=[$(filter --warn-undefined-variables,$(MAKEFLAGS))])$(info AFTER=[$(MISSING_AFTER)])$(file >>seen,DONE)$(info SEEN=[$(file <seen)])
all:;@:
