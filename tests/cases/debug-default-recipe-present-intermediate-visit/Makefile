Makefile:;
$(shell touch -t 202001010000 scratch; touch -t 202001010001 parent)
.DEFAULT:; @$(file >>seen,default:$@)printf 'default:%s\n' '$@'
.INTERMEDIATE: scratch
parent: scratch; @:
