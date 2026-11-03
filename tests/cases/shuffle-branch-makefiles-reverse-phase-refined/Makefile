$(file >seen,anchor)
-include one.mk two.mk
all:;@$(file >>seen,all)$(info all):
one.mk:;@$(file >>seen,one)$(info one):
two.mk:;@$(file >>seen,two)$(info two):
