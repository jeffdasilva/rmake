.PHONY: child a b c
child:a b c
	@$(file >>seen,child)$(info CHILD=[$(filter --shuffle=%,$(MAKEFLAGS))]):
a:;@$(file >>seen,a):
b:;@$(file >>seen,b):
c:;@$(file >>seen,c):
