Makefile:;
$(shell touch -t 202001010002 pool/a; touch -t 202001010002 pool/b; touch -t 202001010001 input)
VPATH := pool
GPATH := pool
a b &: input; @$(file >>seen,group:$@)$(file >pool/a,A)$(file >b,B)printf 'built:%s\n' '$@'
