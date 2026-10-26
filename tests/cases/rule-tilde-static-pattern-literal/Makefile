$(shell python3 setup.py)
unexport HOME
HOME=house
all: house/a.out
~/a.out: ~/%.out: ~/%.in
	@printf 'target=<%s> first=<%s> all=<%s> order=<%s> stem=<%s>\n' '$@' '$<' '$^' '$|' '$*'
