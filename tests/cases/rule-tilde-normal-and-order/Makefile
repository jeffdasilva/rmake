$(shell python3 setup.py)
unexport HOME
HOME=house
all: ~/a | ~/b
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
