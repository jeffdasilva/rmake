Makefile:;
$(shell touch -t 202001010000 a; touch -t 202001010000 b; touch -t 202001010001 trigger)
trigger:;
b: trigger
a b &:; @$(file >>seen,group:$@)$(file >a,A)$(file >b,B)printf 'built:%s\n' '$@'
