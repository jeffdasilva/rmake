Makefile:;
all: middle; @$(file >>seen,expand:$@)printf 'run:%s\n' '$@' >>seen
middle: leaf; @$(file >>seen,expand:$@)printf 'run:%s\n' '$@' >>seen
leaf:; @$(file >>seen,expand:$@)printf 'run:%s\n' '$@' >>seen
