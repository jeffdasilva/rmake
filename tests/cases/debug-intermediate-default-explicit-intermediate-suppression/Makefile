Makefile:;
.DEFAULT:; @$(file >>seen,default:$@)$(file >$@,artifact)printf 'default:%s\n' '$@'
.INTERMEDIATE: scratch
parent: scratch; @$(file >>seen,parent:$(file <scratch))printf 'parent:%s\n' '$(file <scratch)'
scratch:
