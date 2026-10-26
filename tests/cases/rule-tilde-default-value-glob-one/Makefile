$(shell python3 setup.py)
unexport HOME
HOME=house
.DEFAULT_GOAL := ~/*.out
.PHONY: house/a.out
house/a.out:
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
