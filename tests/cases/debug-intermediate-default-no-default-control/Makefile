Makefile:;
.INTERMEDIATE: scratch
parent: scratch; @$(file >>seen,parent:$(file <scratch))printf 'parent:%s\n' '$(file <scratch)'
