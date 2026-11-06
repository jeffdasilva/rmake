Makefile:;
.DEFAULT:; @$(file >>seen,default:$@)$(file >$@,artifact)printf 'default:%s\n' '$@'
.INTERMEDIATE: scratch.out
parent: scratch.out; @:
%.out:; @$(file >>seen,unexpected)printf 'unexpected\n'
