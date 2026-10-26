$(shell python3 setup.py)
unexport HOME
HOME=global
.SECONDEXPANSION:
all: HOME=local
all: x.out
%.out: ~/$$*.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
