$(shell python3 setup.py)
unexport HOME
HOME=homefile
all: ~
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
