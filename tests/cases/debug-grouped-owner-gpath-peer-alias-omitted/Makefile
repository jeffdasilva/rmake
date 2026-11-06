Makefile:;
$(shell touch -t 202001010000 a; touch -t 202001010000 b; touch -t 202001010001 src/input)
VPATH = src
GPATH = src
src/input:;
b: src/input
a b &:; @$(file >>seen,group:$@)$(file >a,A)$(file >b,B)printf 'built:%s\n' '$@'
a: input
