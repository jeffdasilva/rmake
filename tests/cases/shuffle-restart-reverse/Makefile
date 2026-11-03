$(file >>seen,entry)
.PHONY: all a b c
include generated.mk
generated.mk:;@$(file >>seen,remake)printf "# generated\n" > $@
all:a b c
	@$(file >>seen,all)$(info RESTART=[$(MAKE_RESTARTS)]):
a:;@$(file >>seen,a):
b:;@$(file >>seen,b):
c:;@$(file >>seen,c):
