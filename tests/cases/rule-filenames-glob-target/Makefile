$(shell python3 setup.py)
.PHONY: all a.out b.out
all: a.out b.out
*.out:
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
