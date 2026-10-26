$(shell python3 setup.py)
unexport HOME
HOME=house
all: ~/*.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
