$(shell python3 setup.py)
all: x.out
%.out: two\ words.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
