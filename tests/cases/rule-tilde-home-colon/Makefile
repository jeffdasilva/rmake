$(shell python3 setup.py)
unexport HOME
HOME=colon:dir
all: ~/a
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
