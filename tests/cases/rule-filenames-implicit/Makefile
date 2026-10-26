$(shell python3 setup.py)
all: two\ words.out
%.out: %.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
