$(shell python3 setup.py)
unexport HOME
HOME=house
.DEFAULT_GOAL := ~/target
house/target:
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
