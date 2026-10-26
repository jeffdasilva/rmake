$(shell python3 setup.py)
all: a\|b.out
%.out: %.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
