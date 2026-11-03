$(file >seen,anchor)
.PHONY: all t a b c d
all:t
	@$(file >>seen,all)$(info all):
t::a b
	@$(file >>seen,t1)$(info t1=[$+]):
t::c d
	@$(file >>seen,t2)$(info t2=[$+]):
a:;@$(file >>seen,a)$(info a):
b:;@$(file >>seen,b)$(info b):
c:;@$(file >>seen,c)$(info c):
d:;@$(file >>seen,d)$(info d):
