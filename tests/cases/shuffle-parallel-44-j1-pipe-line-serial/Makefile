$(file >seen,anchor)
.PHONY: all a b c
.NOTPARALLEL: all
all: a b c
	@$(file >>seen,all)$(info all PLUS=[$+]):
a:;@$(file >>seen,a)touch a
b:;@$(file >>seen,b)touch b
c:;@$(file >>seen,c)touch c
