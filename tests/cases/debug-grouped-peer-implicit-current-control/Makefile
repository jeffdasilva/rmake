Makefile:;
$(shell touch -t 202001010001 sample.left; touch -t 202001010000 sample.input)
sample.input:;
%.left %.right &: %.input; @$(file >>seen,group:$@)$(file >$*.left,LEFT)$(file >$*.right,RIGHT)printf 'built:%s\n' '$@'
