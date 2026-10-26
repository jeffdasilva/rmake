$(shell python3 setup.py)
all: x.out
%.out: a\|b
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
