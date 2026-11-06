Makefile:;
$(shell touch -t 202001010000 a; touch -t 202001010001 input)
input:;
a: input input; @$(file >>seen,solo:$@)$(file >a,A)printf 'built:%s\n' '$@'
