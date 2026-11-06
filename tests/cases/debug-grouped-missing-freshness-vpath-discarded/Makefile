Makefile:;
$(shell touch -t 202001010000 pool/a; touch -t 202001010001 input)
VPATH := pool
a b &: input; @$(file >>seen,group:$@)$(file >a,A)$(file >b,B)printf 'built:%s\n' '$@'
