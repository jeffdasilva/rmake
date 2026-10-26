$(shell python3 setup.py)
all: two\ words.out
two\ words.out: two\ %.out: %.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
