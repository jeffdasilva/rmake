$(shell python3 setup.py)
.DEFAULT_GOAL := ./two words
two\ words:
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
