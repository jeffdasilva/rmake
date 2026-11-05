Makefile:;
$(shell touch -t 202001010000 sample.left; touch -t 202001010000 sample.right; touch -t 202001010000 sample.input; touch -t 202001010001 trigger)
sample.input:;
trigger:;
sample.right: trigger
%.left %.right &: %.input; @$(file >>seen,group:$@)$(file >$*.left,LEFT)$(file >$*.right,RIGHT)printf 'built:%s\n' '$@'
