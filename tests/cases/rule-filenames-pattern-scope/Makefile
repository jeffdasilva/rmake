$(shell python3 setup.py)
two\ %.out: X=pattern
all: two\ words.out
%.out: %.in;@printf "[%s|%s]\n" "$@" "$(X)"
