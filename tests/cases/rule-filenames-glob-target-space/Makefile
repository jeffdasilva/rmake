$(shell python3 setup.py)
.PHONY: all two\ words.out
all: two\ words.out
*.out:
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
