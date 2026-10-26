$(shell python3 setup.py)
unexport HOME
empty=
HOME=$(empty) $(empty)
all: ~/a
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
