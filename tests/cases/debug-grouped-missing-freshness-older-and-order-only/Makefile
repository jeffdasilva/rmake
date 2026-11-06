Makefile:;
$(shell touch -t 202001010002 a; touch -t 202001010001 input; touch -t 202001010000 witness)
a b &: input | witness; @$(file >>seen,group:$@)$(file >a,A)$(file >b,B)printf 'built:%s\n' '$@'
