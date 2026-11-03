$(file >seen,anchor)
MAKEFLAGS = --no-silent --no-keep-going
.PHONY: all first second
all: first second
first:
	$(file >>seen,first)false
second:
	@$(file >>seen,second)true
